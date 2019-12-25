// Built-in
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::{thread, time};
// External
use ff::{Field, PrimeField};
use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use log::info;
// Workspace deps
use crate::prover_data::ProverData;
use circuit::operation::SignatureData;
use models::merkle_tree::PedersenHasher;
use models::node::tx::PackedPublicKey;
use models::node::{Engine, Fr};

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
}

pub fn maintain(
    conn_pool: storage::ConnectionPool,
    data: Arc<RwLock<ProversDataPool>>,
    rounds_interval: time::Duration,
) {
    info!("preparing prover data routine started");
    let phasher = PedersenHasher::<Engine>::default();
    let params = AltJubjubBn256::new();
    let storage = conn_pool.access_storage().expect("failed to connect to db");
    loop {
        if has_capacity(&data) {
            take_next_commits(&storage, &data).expect("failed to get next commit operations");
        }
        if all_prepared(&data) {
            thread::sleep(rounds_interval);
        } else {
            prepare_next(&storage, &data, &phasher, &params)
                .expect("failed to prepare prover data");
        }
    }
}

fn has_capacity(data: &Arc<RwLock<ProversDataPool>>) -> bool {
    let d = data.write().expect("failed to acquire a lock");
    d.has_capacity()
}

fn take_next_commits(
    storage: &storage::StorageProcessor,
    data: &Arc<RwLock<ProversDataPool>>,
) -> Result<(), String> {
    let d = data.write().expect("failed to acquire a lock");
    let ops = match storage.load_unverified_commits_after_block(d.last_loaded, d.limit) {
        Ok(v) => v,
        Err(e) => {
            return Err(format!("failed to read commit operations: {}", e));
        }
    };
    drop(d);

    if !ops.is_empty() {
        let mut d = data.write().expect("failed to acquire a lock");
        for op in ops.into_iter() {
            let block = op.block.block_number as i64;
            (*d).operations.insert(block, op);
            (*d).last_loaded = block;
        }
    }

    Ok(())
}

fn all_prepared(data: &Arc<RwLock<ProversDataPool>>) -> bool {
    let d = data.read().expect("failed to acquire a lock");
    d.all_prepared()
}

fn prepare_next(
    storage: &storage::StorageProcessor,
    data: &Arc<RwLock<ProversDataPool>>,
    phasher: &PedersenHasher<Engine>,
    params: &AltJubjubBn256,
) -> Result<(), String> {
    let mut d = data.write().expect("failed to acquire a lock");
    let current = (*d).last_prepared + 1;
    let op = d.operations.remove(&current).expect("data is inconsistent");
    drop(d);
    let pd = build_prover_data(&storage, &op, phasher, params)?;
    let mut d = data.write().expect("failed to acquire a lock");
    (*d).last_prepared = current;
    (*d).prepared.insert(op.block.block_number as i64, pd);
    Ok(())
}

fn build_prover_data(
    storage: &storage::StorageProcessor,
    commit_operation: &models::Operation,
    phasher: &PedersenHasher<Engine>,
    params: &AltJubjubBn256,
) -> Result<ProverData, String> {
    info!(
        "building prover data for block {}",
        commit_operation.block.block_number
    );

    let block_number = commit_operation.block.block_number;

    let (_, accounts) = match storage.load_committed_state(Some(block_number - 1)) {
        Ok(v) => v,
        Err(e) => {
            return Err(format!("failed to load commited state: {}", e));
        }
    };
    let mut accounts_tree =
        models::circuit::CircuitAccountTree::new(models::params::account_tree_depth() as u32);
    for acc in accounts {
        let acc_number = acc.0;
        let leaf_copy = models::circuit::account::CircuitAccount::from(acc.1.clone());
        accounts_tree.insert(acc_number, leaf_copy);
    }
    let initial_root = accounts_tree.root_hash();
    let ops = match storage.get_block_operations(block_number) {
        Ok(v) => v,
        Err(e) => {
            return Err(format!("failed to get block operations {}", e));
        }
    };
    // TODO: use conn pool

    circuit::witness::utils::apply_fee(
        &mut accounts_tree,
        commit_operation.block.fee_account,
        0,
        0,
    );
    let mut operations = vec![];
    let mut pub_data = vec![];
    let mut fees = vec![];
    for op in ops {
        match op {
            models::node::FranklinOp::Deposit(deposit) => {
                let deposit_witness =
                    circuit::witness::deposit::apply_deposit_tx(&mut accounts_tree, &deposit);

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
                let transfer_witness =
                    circuit::witness::transfer::apply_transfer_tx(&mut accounts_tree, &transfer);
                let sig_packed = match transfer.tx.signature.signature.serialize_packed() {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(format!("failed to pack transaction signature {}", e));
                    }
                };
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
                fees.push((transfer.tx.fee, transfer.tx.token));
                pub_data.extend(transfer_witness.get_pubdata());
            }
            models::node::FranklinOp::TransferToNew(transfer_to_new) => {
                let transfer_to_new_witness =
                    circuit::witness::transfer_to_new::apply_transfer_to_new_tx(
                        &mut accounts_tree,
                        &transfer_to_new,
                    );
                let sig_packed = match transfer_to_new.tx.signature.signature.serialize_packed() {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(format!("failed to pack transaction signature {}", e));
                    }
                };
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
                fees.push((transfer_to_new.tx.fee, transfer_to_new.tx.token));
                pub_data.extend(transfer_to_new_witness.get_pubdata());
            }
            models::node::FranklinOp::Withdraw(withdraw) => {
                let withdraw_witness =
                    circuit::witness::withdraw::apply_withdraw_tx(&mut accounts_tree, &withdraw);
                let sig_packed = match withdraw.tx.signature.signature.serialize_packed() {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(format!("failed to pack transaction signature {}", e));
                    }
                };
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
                fees.push((withdraw.tx.fee, withdraw.tx.token));
                pub_data.extend(withdraw_witness.get_pubdata());
            }
            models::node::FranklinOp::Close(close) => {
                let close_account_witness = circuit::witness::close_account::apply_close_account_tx(
                    &mut accounts_tree,
                    &close,
                );
                let sig_packed = match close.tx.signature.signature.serialize_packed() {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(format!("failed to pack signature: {}", e));
                    }
                };
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
            models::node::FranklinOp::FullExit(full_exit) => {
                let is_full_exit_success = full_exit.withdraw_amount.is_some();
                let full_exit_witness = circuit::witness::full_exit::apply_full_exit_tx(
                    &mut accounts_tree,
                    &full_exit,
                    is_full_exit_success,
                );

                let r_bits: Vec<_> = models::primitives::bytes_into_be_bits(
                    full_exit.priority_op.signature_r.as_ref(),
                )
                .iter()
                .map(|x| Some(*x))
                .collect();
                let s_bits: Vec<_> = models::primitives::bytes_into_be_bits(
                    full_exit.priority_op.signature_s.as_ref(),
                )
                .iter()
                .map(|x| Some(*x))
                .collect();
                let signature = SignatureData {
                    r_packed: r_bits,
                    s: s_bits,
                };
                let sig_bits: Vec<bool> =
                    models::primitives::bytes_into_be_bits(&full_exit.priority_op.get_bytes());

                let (first_sig_msg, second_sig_msg, third_sig_msg) =
                    circuit::witness::utils::generate_sig_witness(&sig_bits, &phasher, &params);
                let signer_packed_key_bytes = full_exit.priority_op.packed_pubkey.to_vec();
                let signer_packed_key_bits: Vec<_> =
                    models::primitives::bytes_into_be_bits(&signer_packed_key_bytes)
                        .iter()
                        .map(|x| Some(*x))
                        .collect();

                let full_exit_operations =
                    circuit::witness::full_exit::calculate_full_exit_operations_from_witness(
                        &full_exit_witness,
                        &first_sig_msg,
                        &second_sig_msg,
                        &third_sig_msg,
                        &signature,
                        &signer_packed_key_bits,
                    );
                operations.extend(full_exit_operations);
                pub_data.extend(full_exit_witness.get_pubdata(
                    &signature,
                    &models::primitives::bytes_into_be_bits(&signer_packed_key_bytes),
                ));
            }
            _ => {}
        }
    }
    if operations.len() < models::params::block_size_chunks() {
        for _ in 0..models::params::block_size_chunks() - operations.len() {
            let (signature, first_sig_msg, second_sig_msg, third_sig_msg, _sender_x, _sender_y) =
                circuit::witness::utils::generate_dummy_sig_data(&[false], &phasher, &params);
            operations.push(circuit::witness::noop::noop_operation(
                &accounts_tree,
                commit_operation.block.fee_account,
                &first_sig_msg,
                &second_sig_msg,
                &third_sig_msg,
                &signature,
                &[Some(false); 256],
            ));
            pub_data.extend(vec![false; 64]);
        }
    }
    assert_eq!(pub_data.len(), 64 * models::params::block_size_chunks());
    assert_eq!(operations.len(), models::params::block_size_chunks());

    let validator_acc = match accounts_tree.get(commit_operation.block.fee_account as u32) {
        Some(v) => v,
        None => return Err("validator account is absent in the tree".to_owned()),
    };
    let mut validator_balances = vec![];
    for i in 0..1 << models::params::BALANCE_TREE_DEPTH {
        let balance_value = match validator_acc.subtree.get(i as u32) {
            None => Fr::zero(),
            Some(bal) => bal.value,
        };
        validator_balances.push(Some(balance_value));
    }
    let _: Fr = accounts_tree.root_hash();
    let (mut root_after_fee, mut validator_account_witness) = circuit::witness::utils::apply_fee(
        &mut accounts_tree,
        commit_operation.block.fee_account,
        0,
        0,
    );
    for (fee, token) in fees {
        let (root, acc_witness) = circuit::witness::utils::apply_fee(
            &mut accounts_tree,
            commit_operation.block.fee_account,
            u32::from(token),
            match fee.to_string().parse() {
                Ok(v) => v,
                Err(e) => {
                    return Err(format!("failed to parse fee: {}", e));
                }
            },
        );
        root_after_fee = root;
        validator_account_witness = acc_witness;
    }

    assert_eq!(root_after_fee, commit_operation.block.new_root_hash);
    let (validator_audit_path, _) = circuit::witness::utils::get_audits(
        &accounts_tree,
        commit_operation.block.fee_account as u32,
        0,
    );

    let public_data_commitment = circuit::witness::utils::public_data_commitment::<Engine>(
        &pub_data,
        Some(initial_root),
        Some(root_after_fee),
        Some(
            Fr::from_str(&commit_operation.block.fee_account.to_string()).expect("failed to parse"),
        ),
        Some(Fr::from_str(&(block_number).to_string()).expect("failed to parse")),
    );

    Ok(ProverData {
        public_data_commitment,
        old_root: initial_root,
        new_root: commit_operation.block.new_root_hash,
        validator_address: Fr::from_str(&commit_operation.block.fee_account.to_string())
            .expect("failed to parse"),
        operations,
        validator_balances,
        validator_audit_path,
        validator_account: validator_account_witness,
    })
}

type SigData = (Fr, Fr, Fr, SignatureData, Vec<Option<bool>>);

pub fn prepare_sig_data(
    sig_bytes: &[u8],
    tx_bytes: &[u8],
    pub_key: &PackedPublicKey,
) -> Result<SigData, String> {
    let (r_bytes, s_bytes) = sig_bytes.split_at(32);
    let r_bits: Vec<_> = models::primitives::bytes_into_be_bits(&r_bytes)
        .iter()
        .map(|x| Some(*x))
        .collect();
    let s_bits: Vec<_> = models::primitives::bytes_into_be_bits(&s_bytes)
        .iter()
        .map(|x| Some(*x))
        .collect();
    let signature = SignatureData {
        r_packed: r_bits,
        s: s_bits,
    };
    let sig_bits: Vec<bool> = models::primitives::bytes_into_be_bits(&tx_bytes);

    let (first_sig_msg, second_sig_msg, third_sig_msg) =
        circuit::witness::utils::generate_sig_witness(
            &sig_bits,
            &models::params::PEDERSEN_HASHER,
            &models::params::JUBJUB_PARAMS,
        );

    let signer_packed_key_bytes = match pub_key.serialize_packed() {
        Ok(v) => v,
        Err(e) => {
            return Err(format!("failed to prepare signature data: {}", e));
        }
    };
    let signer_packed_key_bits: Vec<_> =
        models::primitives::bytes_into_be_bits(&signer_packed_key_bytes)
            .iter()
            .map(|x| Some(*x))
            .collect();
    Ok((
        first_sig_msg,
        second_sig_msg,
        third_sig_msg,
        signature,
        signer_packed_key_bits,
    ))
}
