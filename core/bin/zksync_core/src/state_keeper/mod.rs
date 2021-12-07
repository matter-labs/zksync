use std::collections::VecDeque;
use std::time::Instant;

// External uses
use futures::{channel::mpsc, stream::StreamExt, SinkExt};
use itertools::Itertools;
use tokio::task::JoinHandle;
// Workspace uses
use zksync_crypto::ff::{PrimeField, PrimeFieldRepr};
use zksync_state::state::{OpSuccess, ZkSyncState};
use zksync_types::{
    block::{
        Block, BlockMetadata, ExecutedOperations, ExecutedPriorityOp, ExecutedTx,
        PendingBlock as SendablePendingBlock,
    },
    gas_counter::GasCounter,
    mempool::SignedTxVariant,
    tx::ZkSyncTx,
    AccountId, Address, PriorityOp, SignedZkSyncTx, H256,
};
// Local uses
use self::{pending_block::PendingBlock, utils::system_time_timestamp};
use crate::{
    committer::{AppliedUpdatesRequest, BlockCommitRequest, CommitRequest},
    mempool::ProposedBlock,
    tx_event_emitter::ProcessedOperations,
};

pub use self::{init_params::ZkSyncStateInitParams, types::StateKeeperRequest};

mod init_params;
mod pending_block;
mod state_restore;
mod types;
mod utils;

#[cfg(test)]
mod tests;

/// Responsible for tx processing and block forming.
pub struct ZkSyncStateKeeper {
    /// Current plasma state
    state: ZkSyncState,

    fee_account_id: AccountId,
    current_unprocessed_priority_op: u64,

    pending_block: PendingBlock,

    rx_for_blocks: mpsc::Receiver<StateKeeperRequest>,
    tx_for_commitments: mpsc::Sender<CommitRequest>,

    available_block_chunk_sizes: Vec<usize>,
    max_miniblock_iterations: usize,
    fast_miniblock_iterations: usize,

    // Two fields below are for optimization: we don't want to overwrite all the block contents over and over.
    // With these fields we'll be able save the diff between two pending block states only.
    /// Amount of succeeded transactions in the pending block at the last pending block synchronization step.
    success_txs_pending_len: usize,
    /// Amount of failed transactions in the pending block at the last pending block synchronization step.
    failed_txs_pending_len: usize,

    /// Channel used for sending queued transaction events. Required since state keeper
    /// has no access to the database.
    processed_tx_events_sender: mpsc::Sender<ProcessedOperations>,
}

impl ZkSyncStateKeeper {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        initial_state: ZkSyncStateInitParams,
        fee_account_address: Address,
        rx_for_blocks: mpsc::Receiver<StateKeeperRequest>,
        tx_for_commitments: mpsc::Sender<CommitRequest>,
        available_block_chunk_sizes: Vec<usize>,
        max_miniblock_iterations: usize,
        fast_miniblock_iterations: usize,
        processed_tx_events_sender: mpsc::Sender<ProcessedOperations>,
    ) -> Self {
        assert!(!available_block_chunk_sizes.is_empty());

        let is_sorted = available_block_chunk_sizes
            .iter()
            .tuple_windows()
            .all(|(a, b)| a < b);
        assert!(is_sorted);

        let state = ZkSyncState::new(
            initial_state.tree,
            initial_state.acc_id_by_addr,
            initial_state.last_block_number + 1,
            initial_state.nfts,
        );

        let (fee_account_id, _) = state
            .get_account_by_address(&fee_account_address)
            .expect("Fee account should be present in the account tree");
        // Keeper starts with the NEXT block
        // we leave space for last tx
        let mut be_bytes = [0u8; 32];
        state
            .root_hash()
            .into_repr()
            .write_be(be_bytes.as_mut())
            .expect("Write commit bytes");
        let previous_root_hash = H256::from(be_bytes);
        let keeper = ZkSyncStateKeeper {
            state,
            fee_account_id,
            current_unprocessed_priority_op: initial_state.unprocessed_priority_op,
            rx_for_blocks,
            tx_for_commitments,
            pending_block: PendingBlock::new(
                initial_state.unprocessed_priority_op,
                &available_block_chunk_sizes,
                previous_root_hash,
                system_time_timestamp(),
            ),
            available_block_chunk_sizes,
            max_miniblock_iterations,
            fast_miniblock_iterations,

            success_txs_pending_len: 0,
            failed_txs_pending_len: 0,
            processed_tx_events_sender,
        };

        let root = keeper.state.root_hash();
        vlog::info!("created state keeper, root hash = {}", root);

        keeper
    }

    async fn initialize(&mut self, pending_block: Option<SendablePendingBlock>) {
        let start = Instant::now();

        if let Some(pending_block) = pending_block {
            // Transform executed operations into non-executed, so they will be executed again.
            // Since it's a pending block, the state updates were not actually applied in the
            // database (as it happens only when full block is committed).
            //
            // We use `apply_tx` and `apply_priority_op` methods directly instead of
            // `apply_txs_batch` to preserve the original execution order. Otherwise there may
            // be a state corruption, if e.g. `Deposit` will be executed before `TransferToNew`
            // and account IDs will change.

            // Sanity check: ensure that we start from a "clean" state.
            if !self.pending_block.failed_txs.is_empty()
                || !self.pending_block.success_operations.is_empty()
            {
                panic!("State keeper was initialized from a dirty state. Pending block was expected to \
                       be empty, but got this instead: \n \
                       Block number: {} \n \
                       Pending block state: {:?}",
                    self.state.block_number, self.pending_block
                );
            }

            // We have to take the timestamp from the pending block, since otherwise already executed
            // transactions may fail because of invalid `valid_from` timestamp.
            self.pending_block.timestamp = pending_block.timestamp;
            self.pending_block.failed_txs = pending_block.failed_txs.clone();

            let mut txs_count = 0;
            let mut priority_op_count = 0;
            let success_operations_count = pending_block.success_operations.len();
            for operation in pending_block.success_operations.clone() {
                match operation {
                    ExecutedOperations::Tx(tx) => {
                        self.apply_tx(&tx.signed_tx)
                            .expect("Tx from the restored pending block was not executed");
                        txs_count += 1;
                    }
                    ExecutedOperations::PriorityOp(op) => {
                        self.apply_priority_op(op.priority_op)
                            .expect("Priority op from the restored pending block was not executed");
                        priority_op_count += 1;
                    }
                }
            }
            self.pending_block.stored_account_updates = self.pending_block.account_updates.len();

            // Sanity check: every transaction we applied should succeed, since we already stored it in the database
            // as successfully executed.
            if success_operations_count != self.pending_block.success_operations.len() {
                panic!(
                    "After execution of pending block some transactions unexpectedly failed:\n \
                    Block number: {} \n \
                    Initial pending block state: {:?}\n \
                    Pending block state: {:?}",
                    self.state.block_number, pending_block, self.pending_block
                );
            }

            vlog::info!(
                "Executed restored proposed block: {} transactions, {} priority operations, {} failed transactions",
                txs_count,
                priority_op_count,
                self.pending_block.failed_txs.len()
            );
        } else {
            vlog::info!("There is no pending block to restore");
        }

        metrics::histogram!("state_keeper.initialize", start.elapsed());
    }

    async fn run(mut self, pending_block: Option<SendablePendingBlock>) {
        self.initialize(pending_block).await;

        while let Some(req) = self.rx_for_blocks.next().await {
            match req {
                StateKeeperRequest::GetAccount(address, sender) => {
                    let account = self.state.get_account_by_address(&address);
                    sender.send(account).unwrap_or_default();
                }
                StateKeeperRequest::GetPendingBlockTimestamp(sender) => {
                    sender
                        .send(self.pending_block.timestamp)
                        .unwrap_or_default();
                }
                StateKeeperRequest::GetLastUnprocessedPriorityOp(sender) => {
                    sender
                        .send(self.current_unprocessed_priority_op)
                        .unwrap_or_default();
                }
                StateKeeperRequest::ExecuteMiniBlock(proposed_block) => {
                    self.execute_proposed_block(proposed_block).await;
                }
                StateKeeperRequest::SealBlock => {
                    self.seal_pending_block().await;
                }
                StateKeeperRequest::GetCurrentState(sender) => {
                    sender.send(self.get_current_state()).unwrap_or_default();
                }
            }
        }
    }

    async fn execute_proposed_block(&mut self, proposed_block: ProposedBlock) {
        let start = Instant::now();
        let mut executed_ops = Vec::new();

        // If pending block is empty we update timestamp
        if self.pending_block.success_operations.is_empty() {
            self.pending_block.timestamp = system_time_timestamp();
        }

        // We want to store this variable before moving anything from the pending block.
        let empty_proposed_block = proposed_block.is_empty();

        let mut priority_op_queue = proposed_block
            .priority_ops
            .into_iter()
            .collect::<VecDeque<_>>();
        while let Some(priority_op) = priority_op_queue.pop_front() {
            match self.apply_priority_op(priority_op) {
                Ok(exec_op) => {
                    executed_ops.push(exec_op);
                }
                Err(priority_op) => {
                    self.seal_pending_block().await;

                    priority_op_queue.push_front(priority_op);
                }
            }
        }

        let mut tx_queue = proposed_block.txs.into_iter().collect::<VecDeque<_>>();
        while let Some(variant) = tx_queue.pop_front() {
            match &variant {
                SignedTxVariant::Tx(tx) => {
                    match self.apply_tx(tx) {
                        Ok(exec_op) => {
                            executed_ops.push(exec_op);
                        }
                        Err(_) => {
                            // We could not execute the tx due to either of block size limit
                            // or the withdraw operations limit, so we seal this block and
                            // the last transaction will go to the next block instead.
                            self.seal_pending_block().await;

                            tx_queue.push_front(variant);
                        }
                    }
                }
                SignedTxVariant::Batch(batch) => {
                    match self.apply_batch(&batch.txs, batch.batch_id) {
                        Ok(mut ops) => {
                            executed_ops.append(&mut ops);
                        }
                        Err(_) => {
                            // We could not execute the batch tx due to either of block size limit
                            // or the withdraw operations limit, so we seal this block and
                            // the last transaction will go to the next block instead.
                            self.seal_pending_block().await;

                            tx_queue.push_front(variant);
                        }
                    }
                }
            }
        }

        if !executed_ops.is_empty() {
            let _ = self
                .processed_tx_events_sender
                .send(ProcessedOperations {
                    block_number: self.state.block_number,
                    executed_ops,
                })
                .await;
        }

        if !self.pending_block.success_operations.is_empty() {
            self.pending_block.pending_block_iteration += 1;
        }

        // If pending block contains withdrawals we seal it faster
        let max_miniblock_iterations = if self.pending_block.fast_processing_required {
            self.fast_miniblock_iterations
        } else {
            self.max_miniblock_iterations
        };
        if self.pending_block.chunks_left == 0
            || self.pending_block.pending_block_iteration > max_miniblock_iterations
        {
            self.seal_pending_block().await;
        } else {
            // We've already incremented the pending block iteration, so this iteration will count towards
            // reaching the block commitment timeout.
            // However, we don't want to pointlessly save the same block again and again.
            if !empty_proposed_block {
                self.store_pending_block().await;
            }
        }

        metrics::histogram!("state_keeper.execute_proposed_block", start.elapsed());
    }

    // Err if there is no space in current block
    fn apply_priority_op(
        &mut self,
        priority_op: PriorityOp,
    ) -> Result<ExecutedOperations, PriorityOp> {
        let start = Instant::now();
        let chunks_needed = priority_op.data.chunks();
        if self.pending_block.chunks_left < chunks_needed {
            return Err(priority_op);
        }

        // Check if adding this transaction to the block won't make the contract operations
        // too expensive.
        let non_executed_op = self
            .state
            .priority_op_to_zksync_op(priority_op.data.clone());
        if self
            .pending_block
            .gas_counter
            .add_op(&non_executed_op)
            .is_err()
        {
            // We've reached the gas limit, seal the block.
            // This transaction will go into the next one.
            return Err(priority_op);
        }

        let OpSuccess {
            fee,
            mut updates,
            executed_op,
        } = self.state.execute_priority_op(priority_op.data.clone());

        self.pending_block.chunks_left -= chunks_needed;
        self.pending_block.account_updates.append(&mut updates);
        if let Some(fee) = fee {
            self.pending_block.collected_fees.push(fee);
        }
        let block_index = self.pending_block.pending_op_block_index;
        self.pending_block.pending_op_block_index += 1;

        let exec_result = ExecutedOperations::PriorityOp(Box::new(ExecutedPriorityOp {
            op: executed_op,
            priority_op,
            block_index,
            created_at: chrono::Utc::now(),
        }));
        self.pending_block
            .success_operations
            .push(exec_result.clone());
        self.current_unprocessed_priority_op += 1;

        metrics::histogram!("state_keeper.apply_priority_op", start.elapsed());
        Ok(exec_result)
    }

    fn apply_batch(
        &mut self,
        txs: &[SignedZkSyncTx],
        batch_id: i64,
    ) -> Result<Vec<ExecutedOperations>, ()> {
        metrics::gauge!("tx_batch_size", txs.len() as f64);
        let start = Instant::now();

        let chunks_needed = self.state.chunks_for_batch(txs);

        // If we can't add the tx to the block due to the size limit, we return this tx,
        // seal the block and execute it again.
        if self.pending_block.chunks_left < chunks_needed {
            return Err(());
        }

        let ops: Vec<_> = txs
            .iter()
            .filter_map(|tx| self.state.zksync_tx_to_zksync_op(tx.tx.clone()).ok())
            .collect();

        let mut executed_operations = Vec::new();

        // If batch doesn't fit into an empty block than we should mark it as failed.
        if !GasCounter::batch_fits_into_empty_block(&ops) {
            let fail_reason = "Amount of gas required to process batch is too big".to_string();
            vlog::warn!("Failed to execute batch: {}", fail_reason);
            for tx in txs {
                let failed_tx = ExecutedTx {
                    signed_tx: tx.clone(),
                    success: false,
                    op: None,
                    fail_reason: Some(fail_reason.clone()),
                    block_index: None,
                    created_at: chrono::Utc::now(),
                    batch_id: Some(batch_id),
                };
                self.pending_block.failed_txs.push(failed_tx.clone());
                let exec_result = ExecutedOperations::Tx(Box::new(failed_tx));
                executed_operations.push(exec_result);
            }
            metrics::histogram!("state_keeper.apply_batch", start.elapsed());
            return Ok(executed_operations);
        }

        // If we can't add the tx to the block due to the gas limit, we return this tx,
        // seal the block and execute it again.
        if !self.pending_block.gas_counter.can_include(&ops) {
            return Err(());
        }

        let all_updates = self
            .state
            .execute_txs_batch(txs, self.pending_block.timestamp);

        for (tx, tx_updates) in txs.iter().zip(all_updates) {
            match tx_updates {
                Ok(OpSuccess {
                    fee,
                    mut updates,
                    executed_op,
                }) => {
                    self.pending_block
                        .gas_counter
                        .add_op(&executed_op)
                        .expect("We have already checked that we can include this tx");

                    self.pending_block.chunks_left -= executed_op.chunks();
                    self.pending_block.account_updates.append(&mut updates);
                    if let Some(fee) = fee {
                        self.pending_block.collected_fees.push(fee);
                    }
                    let block_index = self.pending_block.pending_op_block_index;
                    self.pending_block.pending_op_block_index += 1;

                    let exec_result = ExecutedOperations::Tx(Box::new(ExecutedTx {
                        signed_tx: tx.clone(),
                        success: true,
                        op: Some(executed_op),
                        fail_reason: None,
                        block_index: Some(block_index),
                        created_at: chrono::Utc::now(),
                        batch_id: Some(batch_id),
                    }));
                    self.pending_block
                        .success_operations
                        .push(exec_result.clone());
                    executed_operations.push(exec_result);
                }
                Err(e) => {
                    vlog::warn!("Failed to execute transaction: {:?}, {}", tx, e);
                    let failed_tx = ExecutedTx {
                        signed_tx: tx.clone(),
                        success: false,
                        op: None,
                        fail_reason: Some(e.to_string()),
                        block_index: None,
                        created_at: chrono::Utc::now(),
                        batch_id: Some(batch_id),
                    };
                    self.pending_block.failed_txs.push(failed_tx.clone());
                    let exec_result = ExecutedOperations::Tx(Box::new(failed_tx));
                    executed_operations.push(exec_result);
                }
            };
        }

        metrics::histogram!("state_keeper.apply_batch", start.elapsed());
        Ok(executed_operations)
    }

    fn apply_tx(&mut self, tx: &SignedZkSyncTx) -> Result<ExecutedOperations, ()> {
        let start = Instant::now();
        let chunks_needed = self.state.chunks_for_tx(tx);

        // If we can't add the tx to the block due to the size limit, we return this tx,
        // seal the block and execute it again.
        if self.pending_block.chunks_left < chunks_needed {
            return Err(());
        }

        // Check if adding this transaction to the block won't make the contract operations
        // too expensive.
        let non_executed_op = self.state.zksync_tx_to_zksync_op(tx.tx.clone());
        if let Ok(non_executed_op) = non_executed_op {
            // We only care about successful conversions, since if conversion failed,
            // then transaction will fail as well (as it shares the same code base).
            if !self
                .pending_block
                .gas_counter
                .can_include(&[non_executed_op])
            {
                // We've reached the gas limit, seal the block.
                // This transaction will go into the next one.
                return Err(());
            }
        }

        if let ZkSyncTx::Withdraw(tx) = &tx.tx {
            // Check if we should mark this block as requiring fast processing.
            if tx.fast {
                self.pending_block.fast_processing_required = true;
            }
        }

        let tx_updates = self
            .state
            .execute_tx(tx.tx.clone(), self.pending_block.timestamp);

        let exec_result = match tx_updates {
            Ok(OpSuccess {
                fee,
                mut updates,
                executed_op,
            }) => {
                self.pending_block
                    .gas_counter
                    .add_op(&executed_op)
                    .expect("We have already checked that we can include this tx");

                self.pending_block.chunks_left -= chunks_needed;
                self.pending_block.account_updates.append(&mut updates);
                if let Some(fee) = fee {
                    self.pending_block.collected_fees.push(fee);
                }
                let block_index = self.pending_block.pending_op_block_index;
                self.pending_block.pending_op_block_index += 1;

                let exec_result = ExecutedOperations::Tx(Box::new(ExecutedTx {
                    signed_tx: tx.clone(),
                    success: true,
                    op: Some(executed_op),
                    fail_reason: None,
                    block_index: Some(block_index),
                    created_at: chrono::Utc::now(),
                    batch_id: None,
                }));
                self.pending_block
                    .success_operations
                    .push(exec_result.clone());
                exec_result
            }
            Err(e) => {
                vlog::warn!("Failed to execute transaction: {:?}, {}", tx, e);
                let failed_tx = ExecutedTx {
                    signed_tx: tx.clone(),
                    success: false,
                    op: None,
                    fail_reason: Some(e.to_string()),
                    block_index: None,
                    created_at: chrono::Utc::now(),
                    batch_id: None,
                };
                self.pending_block.failed_txs.push(failed_tx.clone());
                ExecutedOperations::Tx(Box::new(failed_tx))
            }
        };

        metrics::histogram!("state_keeper.apply_tx", start.elapsed());
        Ok(exec_result)
    }

    /// Finalizes the pending block, transforming it into a full block.
    async fn seal_pending_block(&mut self) {
        let start = Instant::now();

        // Apply fees of pending block
        let fee_updates = self
            .state
            .collect_fee(&self.pending_block.collected_fees, self.fee_account_id);
        self.pending_block
            .account_updates
            .extend(fee_updates.into_iter());

        let mut pending_block = std::mem::replace(
            &mut self.pending_block,
            PendingBlock::new(
                self.current_unprocessed_priority_op,
                &self.available_block_chunk_sizes,
                H256::default(),
                system_time_timestamp(),
            ),
        );
        // Once block is sealed, we refresh the counters for the next block.
        self.success_txs_pending_len = 0;
        self.failed_txs_pending_len = 0;

        let mut block_transactions = pending_block.success_operations;
        block_transactions.extend(
            pending_block
                .failed_txs
                .into_iter()
                .map(|tx| ExecutedOperations::Tx(Box::new(tx))),
        );

        let commit_gas_limit = pending_block.gas_counter.commit_gas_limit();
        let verify_gas_limit = pending_block.gas_counter.verify_gas_limit();

        let block = Block::new_from_available_block_sizes(
            self.state.block_number,
            self.state.root_hash(),
            self.fee_account_id,
            block_transactions,
            (
                pending_block.unprocessed_priority_op_before,
                self.current_unprocessed_priority_op,
            ),
            &self.available_block_chunk_sizes,
            commit_gas_limit,
            verify_gas_limit,
            pending_block.previous_block_root_hash,
            pending_block.timestamp,
        );

        self.pending_block.previous_block_root_hash = block.get_eth_encoded_root();

        let block_metadata = BlockMetadata {
            fast_processing: pending_block.fast_processing_required,
        };

        for tx in &block.block_transactions {
            let labels = vec![
                ("stage", "seal_block".to_string()),
                ("name", tx.variance_name()),
                ("token", tx.token_id().to_string()),
            ];
            metrics::increment_counter!("process_tx", &labels);
        }
        let block_commit_request = BlockCommitRequest {
            block,
            block_metadata,
            accounts_updated: pending_block.account_updates.clone(),
        };
        let first_update_order_id = pending_block.stored_account_updates;
        let account_updates = pending_block.account_updates[first_update_order_id..].to_vec();
        let applied_updates_request = AppliedUpdatesRequest {
            account_updates,
            first_update_order_id,
        };
        pending_block.stored_account_updates = pending_block.account_updates.len();
        *self.state.block_number += 1;

        vlog::info!(
            "Creating full block: {}, operations: {}, chunks_left: {}, miniblock iterations: {}",
            *block_commit_request.block.block_number,
            block_commit_request.block.block_transactions.len(),
            pending_block.chunks_left,
            pending_block.pending_block_iteration
        );

        let commit_request = CommitRequest::Block((block_commit_request, applied_updates_request));
        self.tx_for_commitments
            .send(commit_request)
            .await
            .expect("committer receiver dropped");

        metrics::histogram!("state_keeper.seal_pending_block", start.elapsed());
    }

    /// Stores intermediate representation of a pending block in the database,
    /// so the executed transactions are persisted and won't be lost.
    async fn store_pending_block(&mut self) {
        let start = Instant::now();

        // We want include only the newly appeared transactions, since the older ones are already persisted in the
        // database.
        // This is a required optimization, since otherwise time to process the pending block may grow without any
        // limits if we'll be spammed by incorrect transactions (we don't have a limit for an amount of rejected
        // transactions in the block).
        let new_success_operations =
            self.pending_block.success_operations[self.success_txs_pending_len..].to_vec();
        let new_failed_operations =
            self.pending_block.failed_txs[self.failed_txs_pending_len..].to_vec();

        self.success_txs_pending_len = self.pending_block.success_operations.len();
        self.failed_txs_pending_len = self.pending_block.failed_txs.len();

        // Create a pending block object to send.
        // Note that failed operations are not included, as per any operation failure
        // the full block is created immediately.
        let pending_block = SendablePendingBlock {
            number: self.state.block_number,
            chunks_left: self.pending_block.chunks_left,
            unprocessed_priority_op_before: self.pending_block.unprocessed_priority_op_before,
            pending_block_iteration: self.pending_block.pending_block_iteration,
            success_operations: new_success_operations,
            failed_txs: new_failed_operations,
            previous_block_root_hash: self.pending_block.previous_block_root_hash,
            timestamp: self.pending_block.timestamp,
        };
        let first_update_order_id = self.pending_block.stored_account_updates;
        let account_updates = self.pending_block.account_updates[first_update_order_id..].to_vec();
        let applied_updates_request = AppliedUpdatesRequest {
            account_updates,
            first_update_order_id,
        };
        self.pending_block.stored_account_updates = self.pending_block.account_updates.len();

        vlog::debug!(
            "Persisting mini block: {}, operations: {}, failed_txs: {}, chunks_left: {}, miniblock iterations: {}",
            *pending_block.number,
            pending_block.success_operations.len(),
            pending_block.failed_txs.len(),
            pending_block.chunks_left,
            pending_block.pending_block_iteration
        );

        let commit_request = CommitRequest::PendingBlock((pending_block, applied_updates_request));
        self.tx_for_commitments
            .send(commit_request)
            .await
            .expect("committer receiver dropped");
        metrics::histogram!("state_keeper.store_pending_block", start.elapsed());
    }

    pub fn get_current_state(&self) -> ZkSyncStateInitParams {
        ZkSyncStateInitParams {
            tree: self.state.get_balance_tree(),
            acc_id_by_addr: self.state.get_account_addresses(),
            nfts: self.state.nfts.clone(),
            last_block_number: self.state.block_number - 1,
            unprocessed_priority_op: self.current_unprocessed_priority_op,
        }
    }
}

#[must_use]
pub fn start_state_keeper(
    sk: ZkSyncStateKeeper,
    pending_block: Option<SendablePendingBlock>,
) -> JoinHandle<()> {
    tokio::spawn(sk.run(pending_block))
}
