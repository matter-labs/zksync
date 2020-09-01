// Built-in
use std::{thread, time};
// External
use crate::franklin_crypto::bellman::pairing::ff::PrimeField;
use failure::format_err;
use futures::channel::mpsc;
use log::info;
// Workspace deps
use circuit::witness::{
    utils::{SigDataInput, WitnessBuilder},
    ChangePubkeyOffChainWitness, CloseAccountWitness, DepositWitness, FullExitWitness,
    TransferToNewWitness, TransferWitness, WithdrawWitness, Witness,
};
use models::node::block::Block;
use models::params::{account_tree_depth, CHUNK_BIT_WIDTH};
use models::{
    circuit::CircuitAccountTree,
    config_options::ThreadPanicNotify,
    node::{BlockNumber, Fr, FranklinOp},
};
use plasma::state::CollectedFee;
use prover::prover_data::ProverData;
use std::time::Instant;

/// The essential part of this structure is `maintain` function
/// which runs forever and adds data to the database.
///
/// This will generate and store in db witnesses for blocks with indexes
/// start_block, start_block + block_step, start_block + 2*block_step, ...
pub struct WitnessGenerator {
    /// Connection to the database.
    conn_pool: storage::ConnectionPool,
    /// Routine refresh interval.
    rounds_interval: time::Duration,

    start_block: BlockNumber,
    block_step: BlockNumber,
}

enum BlockInfo {
    NotReadyBlock,
    WithWitness,
    NoWitness(Block),
}

impl WitnessGenerator {
    /// Creates a new `WitnessGenerator` object.
    pub fn new(
        conn_pool: storage::ConnectionPool,
        rounds_interval: time::Duration,
        start_block: BlockNumber,
        block_step: BlockNumber,
    ) -> Self {
        Self {
            conn_pool,
            rounds_interval,
            start_block,
            block_step,
        }
    }

    /// Starts the thread running `maintain` method.
    pub fn start(self, panic_notify: mpsc::Sender<bool>) {
        thread::Builder::new()
            .name("prover_server_pool".to_string())
            .spawn(move || {
                let _panic_sentinel = ThreadPanicNotify(panic_notify);
                self.maintain();
            })
            .expect("failed to start provers server");
    }

    /// Returns status of witness for block with index block_number
    fn should_work_on_block(&self, block_number: BlockNumber) -> Result<BlockInfo, failure::Error> {
        let storage = self.conn_pool.access_storage_fragile()?;
        let block = storage.chain().block_schema().get_block(block_number)?;
        let block_info = if let Some(block) = block {
            let witness = storage.prover_schema().get_witness(block_number)?;
            if witness.is_none() {
                BlockInfo::NoWitness(block)
            } else {
                BlockInfo::WithWitness
            }
        } else {
            BlockInfo::NotReadyBlock
        };
        Ok(block_info)
    }

    fn prepare_witness_and_save_it(&self, block: Block) -> Result<(), failure::Error> {
        let timer = Instant::now();
        let storage = self.conn_pool.access_storage_fragile()?;
        let (storage_block_number, mut accounts) = storage
            .chain()
            .state_schema()
            .load_committed_state(Some(block.block_number - 1))
            .map_err(|e| failure::format_err!("couldn't load committed state: {}", e))?;
        trace!(
            "Witness generator get state account {}s",
            timer.elapsed().as_secs()
        );
        failure::ensure!(block.block_number == storage_block_number + 1);

        let timer = Instant::now();
        let mut circuit_account_tree = CircuitAccountTree::new(account_tree_depth());
        for (id, account) in accounts.drain() {
            circuit_account_tree.insert(id, account.into());
        }
        trace!(
            "Witness generator circuit tree insert {}s",
            timer.elapsed().as_secs()
        );

        let timer = Instant::now();
        let witness = build_prover_block_data(&mut circuit_account_tree, &storage, &block)?;
        trace!(
            "Witness generator witness build {}s",
            timer.elapsed().as_secs()
        );

        storage.prover_schema().store_witness(
            block.block_number,
            serde_json::to_value(witness).expect("Witness serialize to json"),
        )?;

        Ok(())
    }

    /// Returns next block for generating witness
    fn next_witness_block(
        current_block: BlockNumber,
        block_step: BlockNumber,
        block_info: &BlockInfo,
    ) -> BlockNumber {
        match block_info {
            BlockInfo::NotReadyBlock => current_block, // Keep waiting
            BlockInfo::WithWitness | BlockInfo::NoWitness(_) => current_block + block_step, // Go to the next block
        }
    }

    /// Updates witness data in database in an infinite loop,
    /// awaiting `rounds_interval` time between updates.
    fn maintain(self) {
        info!(
            "preparing prover data routine started with start_block({}), block_step({})",
            self.start_block, self.block_step
        );
        let mut current_block = self.start_block;
        loop {
            std::thread::sleep(self.rounds_interval);
            let should_work = match self.should_work_on_block(current_block) {
                Ok(should_work) => should_work,
                Err(err) => {
                    log::warn!("witness for block {} check failed: {}", current_block, err);
                    continue;
                }
            };

            let next_block = Self::next_witness_block(current_block, self.block_step, &should_work);
            if let BlockInfo::NoWitness(block) = should_work {
                let block_number = block.block_number;
                if let Err(err) = self.prepare_witness_and_save_it(block) {
                    log::warn!("Witness generator ({},{}) failed to prepare witness for block: {}, err: {}",
                        self.start_block, self.block_step, block_number, err);
                    continue; // Retry the same block on the next iteration.
                }
            }

            // Update current block.
            current_block = next_block;
        }
    }
}

fn build_prover_block_data(
    account_tree: &mut CircuitAccountTree,
    storage: &storage::StorageProcessor,
    block: &Block,
) -> Result<ProverData, failure::Error> {
    let block_number = block.block_number;
    let block_size = block.block_chunks_size;

    info!("building prover data for block {}", &block_number);

    let mut witness_accum = WitnessBuilder::new(account_tree, block.fee_account, block_number);

    let ops = storage
        .chain()
        .block_schema()
        .get_block_operations(block_number)
        .map_err(|e| failure::format_err!("failed to get block operations {}", e))?;

    let mut operations = vec![];
    let mut pub_data = vec![];
    let mut fees = vec![];
    for op in ops {
        match op {
            FranklinOp::Deposit(deposit) => {
                let deposit_witness =
                    DepositWitness::apply_tx(&mut witness_accum.account_tree, &deposit);

                let deposit_operations = deposit_witness.calculate_operations(());
                operations.extend(deposit_operations);
                pub_data.extend(deposit_witness.get_pubdata());
            }
            FranklinOp::Transfer(transfer) => {
                let transfer_witness =
                    TransferWitness::apply_tx(&mut witness_accum.account_tree, &transfer);

                let input =
                    SigDataInput::from_transfer_op(&transfer).map_err(|e| format_err!("{}", e))?;
                let transfer_operations = transfer_witness.calculate_operations(input);

                operations.extend(transfer_operations);
                fees.push(CollectedFee {
                    token: transfer.tx.token,
                    amount: transfer.tx.fee,
                });
                pub_data.extend(transfer_witness.get_pubdata());
            }
            FranklinOp::TransferToNew(transfer_to_new) => {
                let transfer_to_new_witness = TransferToNewWitness::apply_tx(
                    &mut witness_accum.account_tree,
                    &transfer_to_new,
                );

                let input = SigDataInput::from_transfer_to_new_op(&transfer_to_new)
                    .map_err(|e| format_err!("{}", e))?;
                let transfer_to_new_operations =
                    transfer_to_new_witness.calculate_operations(input);

                operations.extend(transfer_to_new_operations);
                fees.push(CollectedFee {
                    token: transfer_to_new.tx.token,
                    amount: transfer_to_new.tx.fee,
                });
                pub_data.extend(transfer_to_new_witness.get_pubdata());
            }
            FranklinOp::Withdraw(withdraw) => {
                let withdraw_witness =
                    WithdrawWitness::apply_tx(&mut witness_accum.account_tree, &withdraw);

                let input =
                    SigDataInput::from_withdraw_op(&withdraw).map_err(|e| format_err!("{}", e))?;
                let withdraw_operations = withdraw_witness.calculate_operations(input);

                operations.extend(withdraw_operations);
                fees.push(CollectedFee {
                    token: withdraw.tx.token,
                    amount: withdraw.tx.fee,
                });
                pub_data.extend(withdraw_witness.get_pubdata());
            }
            FranklinOp::Close(close) => {
                let close_account_witness =
                    CloseAccountWitness::apply_tx(&mut witness_accum.account_tree, &close);

                let input =
                    SigDataInput::from_close_op(&close).map_err(|e| format_err!("{}", e))?;
                let close_account_operations = close_account_witness.calculate_operations(input);

                operations.extend(close_account_operations);
                pub_data.extend(close_account_witness.get_pubdata());
            }
            FranklinOp::FullExit(full_exit_op) => {
                let success = full_exit_op.withdraw_amount.is_some();

                let full_exit_witness = FullExitWitness::apply_tx(
                    &mut witness_accum.account_tree,
                    &(*full_exit_op, success),
                );

                let full_exit_operations = full_exit_witness.calculate_operations(());

                operations.extend(full_exit_operations);
                pub_data.extend(full_exit_witness.get_pubdata());
            }
            FranklinOp::ChangePubKeyOffchain(change_pkhash_op) => {
                let change_pkhash_witness = ChangePubkeyOffChainWitness::apply_tx(
                    &mut witness_accum.account_tree,
                    &change_pkhash_op,
                );

                let change_pkhash_operations = change_pkhash_witness.calculate_operations(());

                operations.extend(change_pkhash_operations);
                pub_data.extend(change_pkhash_witness.get_pubdata());
            }
            FranklinOp::Noop(_) => {} // Noops are handled below
        }
    }

    witness_accum.add_operation_with_pubdata(operations, pub_data);
    witness_accum.extend_pubdata_with_noops(block_size);
    assert_eq!(witness_accum.pubdata.len(), CHUNK_BIT_WIDTH * block_size);
    assert_eq!(witness_accum.operations.len(), block_size);

    witness_accum.collect_fees(&fees);
    assert_eq!(
        witness_accum
            .root_after_fees
            .expect("root_after_fees not present"),
        block.new_root_hash
    );
    witness_accum.calculate_pubdata_commitment();

    Ok(ProverData {
        public_data_commitment: witness_accum.pubdata_commitment.unwrap(),
        old_root: witness_accum.initial_root_hash,
        initial_used_subtree_root: witness_accum.initial_used_subtree_root_hash,
        new_root: block.new_root_hash,
        validator_address: Fr::from_str(&block.fee_account.to_string()).expect("failed to parse"),
        operations: witness_accum.operations,
        validator_balances: witness_accum.fee_account_balances.unwrap(),
        validator_audit_path: witness_accum.fee_account_audit_path.unwrap(),
        validator_account: witness_accum.fee_account_witness.unwrap(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::node::Fr;
    use web3::types::U256;

    #[test]
    fn test_next_witness_block() {
        assert_eq!(
            WitnessGenerator::next_witness_block(3, 4, &BlockInfo::NotReadyBlock),
            3
        );
        assert_eq!(
            WitnessGenerator::next_witness_block(3, 4, &BlockInfo::WithWitness),
            7
        );
        let empty_block = Block::new(
            0,
            Fr::default(),
            0,
            vec![],
            (0, 0),
            0,
            U256::default(),
            U256::default(),
        );
        assert_eq!(
            WitnessGenerator::next_witness_block(3, 4, &BlockInfo::NoWitness(empty_block)),
            7
        );
    }
}
