// Built-in
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::{thread, time};
// External
use crate::franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use crate::franklin_crypto::bellman::pairing::ff::{Field, PrimeField};
use log::info;
// Workspace deps
use circuit::operation::SignatureData;
use circuit::witness::change_pubkey_offchain::{
    apply_change_pubkey_offchain_tx, calculate_change_pubkey_offchain_from_witness,
};
use circuit::witness::full_exit::{
    apply_full_exit_tx, calculate_full_exit_operations_from_witness,
};
use circuit::witness::test_utils::WitnessAccumulator;
use circuit::witness::utils::prepare_sig_data;
use models::merkle_tree::PedersenHasher;
use models::node::{Engine, Fr};
use plasma::state::CollectedFee;
use prover::prover_data::ProverData;

pub struct ProversDataPool {
    last_prepared: i64,
    last_loaded: i64,
    limit: i64,
    operations: HashMap<i64, models::Operation>,
    prepared: HashMap<i64, ProverData>,
}

impl ProversDataPool {
    pub fn new() -> Self {
        ProversDataPool {
            last_prepared: 0,
            last_loaded: 0,
            limit: 10,
            operations: HashMap::new(),
            prepared: HashMap::new(),
        }
    }

    pub fn get(&self, block: i64) -> Option<&ProverData> {
        self.prepared.get(&block)
    }

    pub fn clean_up(&mut self, block: i64) {
        self.prepared.remove(&block);
    }

    fn has_capacity(&self) -> bool {
        self.last_loaded - self.last_prepared + (self.prepared.len() as i64) < self.limit as i64
    }

    fn all_prepared(&self) -> bool {
        self.last_loaded == self.last_prepared
    }

    fn store_to_prove(&mut self, op: models::Operation) {
        let block = op.block.block_number as i64;
        self.last_loaded = block;
        self.operations.insert(block, op);
    }

    fn take_next_to_prove(&mut self) -> Result<models::Operation, String> {
        if self.last_prepared == 0 {
            // Pool restart or first ever take.
            // Handling restart case by setting proper value for `last_prepared`.
            let mut first_from_loaded = 0;
            for key in self.operations.keys() {
                if first_from_loaded == 0 || *key < first_from_loaded {
                    first_from_loaded = *key;
                }
            }
            self.last_prepared = first_from_loaded - 1
        }
        let next = self.last_prepared + 1;
        match self.operations.remove(&next) {
            Some(v) => Ok(v),
            None => Err("data is inconsistent".to_owned()),
        }
    }
}

pub fn maintain(
    conn_pool: storage::ConnectionPool,
    data: Arc<RwLock<ProversDataPool>>,
    rounds_interval: time::Duration,
) {
    info!("preparing prover data routine started");
    let phasher = PedersenHasher::<Engine>::default();
    let params = AltJubjubBn256::new();
    loop {
        if has_capacity(&data) {
            take_next_commits(&conn_pool, &data).expect("failed to get next commit operations");
        }
        if all_prepared(&data) {
            thread::sleep(rounds_interval);
        } else {
            prepare_next(&conn_pool, &data, &phasher, &params)
                .expect("failed to prepare prover data");
        }
    }
}

fn has_capacity(data: &Arc<RwLock<ProversDataPool>>) -> bool {
    let d = data.write().expect("failed to acquire a lock");
    d.has_capacity()
}

fn take_next_commits(
    conn_pool: &storage::ConnectionPool,
    data: &Arc<RwLock<ProversDataPool>>,
) -> Result<(), String> {
    let ops = {
        let d = data.write().expect("failed to acquire a lock");
        let storage = conn_pool.access_storage().expect("failed to connect to db");
        storage
            .load_unverified_commits_after_block(d.last_loaded, d.limit)
            .map_err(|e| format!("failed to read commit operations: {}", e))?
    };

    if !ops.is_empty() {
        let mut d = data.write().expect("failed to acquire a lock");
        for op in ops.into_iter() {
            (*d).store_to_prove(op)
        }
    }

    Ok(())
}

fn all_prepared(data: &Arc<RwLock<ProversDataPool>>) -> bool {
    let d = data.read().expect("failed to acquire a lock");
    d.all_prepared()
}

fn prepare_next(
    conn_pool: &storage::ConnectionPool,
    data: &Arc<RwLock<ProversDataPool>>,
    phasher: &PedersenHasher<Engine>,
    params: &AltJubjubBn256,
) -> Result<(), String> {
    let op = {
        let mut d = data.write().expect("failed to acquire a lock");
        d.take_next_to_prove()?
    };
    let storage = conn_pool.access_storage().expect("failed to connect to db");
    let pd = build_prover_data(&storage, &op, phasher, params)?;
    let mut d = data.write().expect("failed to acquire a lock");
    (*d).last_prepared += 1;
    (*d).prepared.insert(op.block.block_number as i64, pd);
    Ok(())
}

fn build_prover_data(
    storage: &storage::StorageProcessor,
    commit_operation: &models::Operation,
    phasher: &PedersenHasher<Engine>,
    params: &AltJubjubBn256,
) -> Result<ProverData, String> {
    let block_number = commit_operation.block.block_number;

    info!("building prover data for block {}", &block_number);

    let accounts_tree = {
        let (_, accounts) = storage
            .load_committed_state(Some(block_number - 1))
            .map_err(|e| format!("failed to load commited state: {}", e))?;
        let mut accounts_tree =
            models::circuit::CircuitAccountTree::new(models::params::account_tree_depth() as u32);
        for (acc_number, account) in accounts {
            let leaf_copy = models::circuit::account::CircuitAccount::from(account.clone());
            accounts_tree.insert(acc_number, leaf_copy);
        }
        accounts_tree
    };

    let mut witness_accum = WitnessAccumulator::new(
        accounts_tree,
        commit_operation.block.fee_account,
        block_number,
    );

    let initial_root = witness_accum.account_tree.root_hash();
    let ops = storage
        .get_block_operations(block_number)
        .map_err(|e| format!("failed to get block operations {}", e))?;

    // circuit::witness::utils::apply_fee(
    //     &mut witness_accum.account_tree,
    //     commit_operation.block.fee_account,
    //     0,
    //     0,
    // );
    let mut operations = vec![];
    let mut pub_data = vec![];
    let mut fees = vec![];
    for op in ops {
        match op {
            models::node::FranklinOp::Deposit(deposit) => {
                let deposit_witness = circuit::witness::deposit::apply_deposit_tx(
                    &mut witness_accum.account_tree,
                    &deposit,
                );

                let deposit_operations =
                    circuit::witness::deposit::calculate_deposit_operations_from_witness(
                        &deposit_witness,
                        &Fr::zero(),
                        &Fr::zero(),
                        &Fr::zero(),
                        &SignatureData {
                            r_packed: vec![Some(false); 256],
                            s: vec![Some(false); 256],
                        },
                        &[Some(false); 256], //doesn't matter for deposit
                    );
                operations.extend(deposit_operations);
                pub_data.extend(deposit_witness.get_pubdata());
            }
            models::node::FranklinOp::Transfer(transfer) => {
                let transfer_witness = circuit::witness::transfer::apply_transfer_tx(
                    &mut witness_accum.account_tree,
                    &transfer,
                );
                let sig_packed = transfer
                    .tx
                    .signature
                    .signature
                    .serialize_packed()
                    .map_err(|e| format!("failed to pack transaction signature {}", e))?;
                let (
                    first_sig_msg,
                    second_sig_msg,
                    third_sig_msg,
                    signature_data,
                    signer_packed_key_bits,
                ) = prepare_sig_data(
                    &sig_packed,
                    &transfer.tx.get_bytes(),
                    &transfer.tx.signature.pub_key,
                )?;
                let transfer_operations =
                    circuit::witness::transfer::calculate_transfer_operations_from_witness(
                        &transfer_witness,
                        &first_sig_msg,
                        &second_sig_msg,
                        &third_sig_msg,
                        &signature_data,
                        &signer_packed_key_bits,
                    );
                operations.extend(transfer_operations);
                fees.push(CollectedFee {
                    token: transfer.tx.token,
                    amount: transfer.tx.fee,
                });
                pub_data.extend(transfer_witness.get_pubdata());
            }
            models::node::FranklinOp::TransferToNew(transfer_to_new) => {
                let transfer_to_new_witness =
                    circuit::witness::transfer_to_new::apply_transfer_to_new_tx(
                        &mut witness_accum.account_tree,
                        &transfer_to_new,
                    );
                let sig_packed = transfer_to_new
                    .tx
                    .signature
                    .signature
                    .serialize_packed()
                    .map_err(|e| format!("failed to pack transaction signature {}", e))?;
                let (
                    first_sig_msg,
                    second_sig_msg,
                    third_sig_msg,
                    signature_data,
                    signer_packed_key_bits,
                ) = prepare_sig_data(
                    &sig_packed,
                    &transfer_to_new.tx.get_bytes(),
                    &transfer_to_new.tx.signature.pub_key,
                )?;

                let transfer_to_new_operations =
                    circuit::witness::transfer_to_new::calculate_transfer_to_new_operations_from_witness(
                        &transfer_to_new_witness,
                        &first_sig_msg,
                        &second_sig_msg,
                        &third_sig_msg,
                        &signature_data,
                        &signer_packed_key_bits,
                    );
                operations.extend(transfer_to_new_operations);
                fees.push(CollectedFee {
                    token: transfer_to_new.tx.token,
                    amount: transfer_to_new.tx.fee,
                });
                pub_data.extend(transfer_to_new_witness.get_pubdata());
            }
            models::node::FranklinOp::Withdraw(withdraw) => {
                let withdraw_witness = circuit::witness::withdraw::apply_withdraw_tx(
                    &mut witness_accum.account_tree,
                    &withdraw,
                );
                let sig_packed = withdraw
                    .tx
                    .signature
                    .signature
                    .serialize_packed()
                    .map_err(|e| format!("failed to pack transaction signature {}", e))?;
                let (
                    first_sig_msg,
                    second_sig_msg,
                    third_sig_msg,
                    signature_data,
                    signer_packed_key_bits,
                ) = prepare_sig_data(
                    &sig_packed,
                    &withdraw.tx.get_bytes(),
                    &withdraw.tx.signature.pub_key,
                )?;

                let withdraw_operations =
                    circuit::witness::withdraw::calculate_withdraw_operations_from_witness(
                        &withdraw_witness,
                        &first_sig_msg,
                        &second_sig_msg,
                        &third_sig_msg,
                        &signature_data,
                        &signer_packed_key_bits,
                    );
                operations.extend(withdraw_operations);
                fees.push(CollectedFee {
                    token: withdraw.tx.token,
                    amount: withdraw.tx.fee,
                });
                pub_data.extend(withdraw_witness.get_pubdata());
            }
            models::node::FranklinOp::Close(close) => {
                let close_account_witness = circuit::witness::close_account::apply_close_account_tx(
                    &mut witness_accum.account_tree,
                    &close,
                );
                let sig_packed = close
                    .tx
                    .signature
                    .signature
                    .serialize_packed()
                    .map_err(|e| format!("failed to pack signature: {}", e))?;
                let (
                    first_sig_msg,
                    second_sig_msg,
                    third_sig_msg,
                    signature_data,
                    signer_packed_key_bits,
                ) = prepare_sig_data(
                    &sig_packed,
                    &close.tx.get_bytes(),
                    &close.tx.signature.pub_key,
                )?;

                let close_account_operations =
                    circuit::witness::close_account::calculate_close_account_operations_from_witness(
                        &close_account_witness,
                        &first_sig_msg,
                        &second_sig_msg,
                        &third_sig_msg,
                        &signature_data,
                        &signer_packed_key_bits,
                    );
                operations.extend(close_account_operations);
                pub_data.extend(close_account_witness.get_pubdata());
            }
            models::node::FranklinOp::FullExit(full_exit_op) => {
                let success = full_exit_op.withdraw_amount.is_some();
                let full_exit_witness =
                    apply_full_exit_tx(&mut witness_accum.account_tree, &full_exit_op, success);
                let full_exit_operations =
                    calculate_full_exit_operations_from_witness(&full_exit_witness);
                operations.extend(full_exit_operations);
                pub_data.extend(full_exit_witness.get_pubdata());
            }
            models::node::FranklinOp::ChangePubKeyOffchain(change_pkhash_op) => {
                let change_pkhash_witness = apply_change_pubkey_offchain_tx(
                    &mut witness_accum.account_tree,
                    &change_pkhash_op,
                );
                let change_pkhash_operations =
                    calculate_change_pubkey_offchain_from_witness(&change_pkhash_witness);
                operations.extend(change_pkhash_operations);
                pub_data.extend(change_pkhash_witness.get_pubdata());
            }
            models::node::FranklinOp::Noop(_) => {} // Noops are handled below
        }
    }

    witness_accum.add_operation_with_pubdata(operations, pub_data);
    witness_accum.extend_pubdata_with_noops(phasher, params);
    assert_eq!(
        witness_accum.pubdata.len(),
        64 * models::params::block_size_chunks()
    );
    assert_eq!(
        witness_accum.operations.len(),
        models::params::block_size_chunks()
    );

    witness_accum.collect_fees(&fees);
    assert_eq!(
        witness_accum
            .root_after_fees
            .expect("root_after_fees not present"),
        commit_operation.block.new_root_hash
    );
    witness_accum.calculate_pubdata_commitment();

    Ok(ProverData {
        public_data_commitment: witness_accum.pubdata_commitment.unwrap(),
        old_root: initial_root,
        new_root: commit_operation.block.new_root_hash,
        validator_address: Fr::from_str(&commit_operation.block.fee_account.to_string())
            .expect("failed to parse"),
        operations: witness_accum.operations,
        validator_balances: witness_accum.fee_account_balances.unwrap(),
        validator_audit_path: witness_accum.fee_account_audit_path.unwrap(),
        validator_account: witness_accum.fee_account_witness.unwrap(),
    })
}
