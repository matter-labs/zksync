use std::collections::VecDeque;
use std::time::Instant;

// External uses
use futures::{channel::mpsc, stream::StreamExt, SinkExt};
use tokio::task::JoinHandle;
// Workspace uses
use zksync_crypto::ff::{PrimeField, PrimeFieldRepr};
use zksync_state::state::{OpSuccess, ZkSyncState};
use zksync_types::{
    block::{
        BlockMetadata, ExecutedOperations, ExecutedPriorityOp, ExecutedTx, IncompleteBlock,
        PendingBlock as SendablePendingBlock,
    },
    gas_counter::GasCounter,
    mempool::SignedTxVariant,
    tx::ZkSyncTx,
    Address, PriorityOp, SignedZkSyncTx, H256,
};
// Local uses
use self::{
    pending_block::PendingBlock,
    root_hash_calculator::{BlockRootHashJob, BlockRootHashJobQueue, RootHashCalculator},
    types::{ApplyOutcome, StateKeeperConfig},
    utils::system_time_timestamp,
};
use crate::{
    committer::{BlockCommitRequest, CommitRequest},
    mempool::ProposedBlock,
    tx_event_emitter::ProcessedOperations,
};

pub use self::{init_params::ZkSyncStateInitParams, types::StateKeeperRequest};

mod init_params;
mod pending_block;
mod root_hash_calculator;
mod state_restore;
mod types;
mod utils;

#[cfg(test)]
mod tests;

/// Responsible for tx processing and block forming.
pub struct ZkSyncStateKeeper {
    /// Current plasma state
    state: ZkSyncState,
    pending_block: PendingBlock,
    config: StateKeeperConfig,

    rx_for_blocks: mpsc::Receiver<StateKeeperRequest>,
    tx_for_commitments: mpsc::Sender<CommitRequest>,
    /// Channel used for sending queued transaction events. Required since state keeper
    /// has no access to the database.
    processed_tx_events_sender: mpsc::Sender<ProcessedOperations>,

    /// Queue for root hash calculator.
    /// Contains blocks that were sealed but for which root hash has not been calculated yet.
    root_hash_queue: BlockRootHashJobQueue,
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
        let current_block = initial_state.last_block_number + 1;
        let state = ZkSyncState::new(
            initial_state.tree,
            initial_state.acc_id_by_addr,
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

        let config = StateKeeperConfig::new(
            fee_account_id,
            available_block_chunk_sizes,
            max_miniblock_iterations,
            fast_miniblock_iterations,
        );

        let keeper = ZkSyncStateKeeper {
            state,
            pending_block: PendingBlock::new(
                current_block,
                initial_state.unprocessed_priority_op,
                config.max_block_size(),
                previous_root_hash,
                system_time_timestamp(),
            ),
            config,

            rx_for_blocks,
            tx_for_commitments,
            processed_tx_events_sender,

            root_hash_queue: BlockRootHashJobQueue::new(),
        };

        todo!("Put all the incomplete jobs into queue");

        keeper
    }

    // TODO (ZKS-821): We should get rid of this function and create state keeper in a ready-to-go state.
    // Currently we partially initialize state keeper, and then finalize initialization when it's actually started
    // which is not a good practice.
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
            if !self.pending_block.is_empty() {
                panic!("State keeper was initialized from a dirty state. Pending block was expected to \
                       be empty, but got this instead: \n \
                       Block number: {} \n \
                       Pending block state: {:?}",
                    self.pending_block.number, self.pending_block
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
                            .assert_included("Tx from the restored pending block was not executed");
                        txs_count += 1;
                    }
                    ExecutedOperations::PriorityOp(op) => {
                        self.apply_priority_op(&op.priority_op).assert_included(
                            "Priority op from the restored pending block was not executed",
                        );
                        priority_op_count += 1;
                    }
                }
            }

            // After we executed transactions, we may renew counters for already sent operations.
            // These updates were already processed, as we've loaded the block from the database.
            self.pending_block.stored_account_updates = self.pending_block.account_updates.len();

            // Sanity check: every transaction we applied should succeed, since we already stored it in the database
            // as successfully executed.
            if success_operations_count != self.pending_block.success_operations.len() {
                panic!(
                    "After execution of pending block some transactions unexpectedly failed:\n \
                    Block number: {} \n \
                    Initial pending block state: {:?}\n \
                    Pending block state: {:?}",
                    self.pending_block.number, pending_block, self.pending_block
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
                // TODO (ZKS-821): Only used by block proposer, can be removed.
                StateKeeperRequest::GetLastUnprocessedPriorityOp(sender) => {
                    sender
                        .send(self.pending_block.unprocessed_priority_op_current)
                        .unwrap_or_default();
                }
                StateKeeperRequest::ExecuteMiniBlock(proposed_block) => {
                    self.execute_proposed_block(proposed_block).await;
                }
                // TODO (ZKS-821): Only used by testkit, should be removed.
                StateKeeperRequest::SealBlock => {
                    self.seal_pending_block().await;
                }
                // TODO (ZKS-821): Only used by testkit, should be removed.
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
            match self.apply_priority_op(&priority_op) {
                ApplyOutcome::Included(exec_op) => {
                    executed_ops.push(exec_op);
                }
                ApplyOutcome::NotIncluded => {
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
                        ApplyOutcome::Included(exec_op) => {
                            executed_ops.push(exec_op);
                        }
                        ApplyOutcome::NotIncluded => {
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
                        ApplyOutcome::Included(mut ops) => {
                            executed_ops.append(&mut ops);
                        }
                        ApplyOutcome::NotIncluded => {
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

        // TODO (ZKS-821): We can store events in `committer` (as it's already responsible for applying results of
        // the block execution), there is no need in additional actor for that.
        if !executed_ops.is_empty() {
            let _ = self
                .processed_tx_events_sender
                .send(ProcessedOperations {
                    block_number: self.pending_block.number,
                    executed_ops,
                })
                .await;
        }

        // Iteration is complete, increment it in the pending block.
        self.pending_block.increment_iteration();

        // If pending block contains withdrawals we seal it faster
        let max_miniblock_iterations = if self.pending_block.fast_processing_required {
            self.config.fast_miniblock_iterations
        } else {
            self.config.max_miniblock_iterations
        };
        if self.pending_block.should_seal(max_miniblock_iterations) {
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
    fn apply_priority_op(&mut self, priority_op: &PriorityOp) -> ApplyOutcome<ExecutedOperations> {
        let start = Instant::now();
        let chunks_needed = priority_op.data.chunks();
        if self.pending_block.chunks_left < chunks_needed {
            return ApplyOutcome::NotIncluded;
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
            return ApplyOutcome::NotIncluded;
        }

        let OpSuccess {
            fee,
            updates,
            executed_op,
        } = self.state.execute_priority_op(priority_op.data.clone());
        let block_index = self.pending_block.pending_op_block_index;

        let exec_result = ExecutedOperations::PriorityOp(Box::new(ExecutedPriorityOp {
            op: executed_op,
            priority_op: priority_op.clone(),
            block_index,
            created_at: chrono::Utc::now(),
        }));

        self.pending_block.add_successful_execution(
            chunks_needed,
            updates,
            fee,
            exec_result.clone(),
        );

        metrics::histogram!("state_keeper.apply_priority_op", start.elapsed());
        ApplyOutcome::Included(exec_result)
    }

    fn apply_batch(
        &mut self,
        txs: &[SignedZkSyncTx],
        batch_id: i64,
    ) -> ApplyOutcome<Vec<ExecutedOperations>> {
        metrics::gauge!("tx_batch_size", txs.len() as f64);
        let start = Instant::now();

        let chunks_needed = self.state.chunks_for_batch(txs);

        // If we can't add the tx to the block due to the size limit, we return this tx,
        // seal the block and execute it again.
        if self.pending_block.chunks_left < chunks_needed {
            return ApplyOutcome::NotIncluded;
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
            return ApplyOutcome::Included(executed_operations);
        }

        // If we can't add the tx to the block due to the gas limit, we return this tx,
        // seal the block and execute it again.
        if !self.pending_block.gas_counter.can_include(&ops) {
            return ApplyOutcome::NotIncluded;
        }

        let all_updates = self
            .state
            .execute_txs_batch(txs, self.pending_block.timestamp);

        for (tx, tx_updates) in txs.iter().zip(all_updates) {
            match tx_updates {
                Ok(OpSuccess {
                    fee,
                    updates,
                    executed_op,
                }) => {
                    self.pending_block
                        .gas_counter
                        .add_op(&executed_op)
                        .expect("We have already checked that we can include this tx");
                    let chunks_used = executed_op.chunks();

                    let block_index = self.pending_block.pending_op_block_index;
                    let exec_result = ExecutedOperations::Tx(Box::new(ExecutedTx {
                        signed_tx: tx.clone(),
                        success: true,
                        op: Some(executed_op),
                        fail_reason: None,
                        block_index: Some(block_index),
                        created_at: chrono::Utc::now(),
                        batch_id: Some(batch_id),
                    }));

                    self.pending_block.add_successful_execution(
                        chunks_used,
                        updates,
                        fee,
                        exec_result.clone(),
                    );

                    executed_operations.push(exec_result);
                }
                Err(e) => {
                    vlog::warn!("Failed to execute transaction: {:?}, {}", tx, e);

                    let labels = vec![
                        ("stage", "state".to_string()),
                        ("error", e.reason.to_string()),
                    ];
                    metrics::increment_counter!("rejected_txs", &labels);

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
        ApplyOutcome::Included(executed_operations)
    }

    fn apply_tx(&mut self, tx: &SignedZkSyncTx) -> ApplyOutcome<ExecutedOperations> {
        let start = Instant::now();
        let chunks_needed = self.state.chunks_for_tx(tx);

        // If we can't add the tx to the block due to the size limit, we return this tx,
        // seal the block and execute it again.
        if self.pending_block.chunks_left < chunks_needed {
            return ApplyOutcome::NotIncluded;
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
                return ApplyOutcome::NotIncluded;
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
                updates,
                executed_op,
            }) => {
                self.pending_block
                    .gas_counter
                    .add_op(&executed_op)
                    .expect("We have already checked that we can include this tx");

                let block_index = self.pending_block.pending_op_block_index;
                let exec_result = ExecutedOperations::Tx(Box::new(ExecutedTx {
                    signed_tx: tx.clone(),
                    success: true,
                    op: Some(executed_op),
                    fail_reason: None,
                    block_index: Some(block_index),
                    created_at: chrono::Utc::now(),
                    batch_id: None,
                }));

                self.pending_block.add_successful_execution(
                    chunks_needed,
                    updates,
                    fee,
                    exec_result.clone(),
                );

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
                let labels = vec![("stage", "state".to_string()), ("error", e.to_string())];
                metrics::increment_counter!("rejected_txs", &labels);
                self.pending_block.failed_txs.push(failed_tx.clone());
                ExecutedOperations::Tx(Box::new(failed_tx))
            }
        };

        metrics::histogram!("state_keeper.apply_tx", start.elapsed());
        ApplyOutcome::Included(exec_result)
    }

    /// Finalizes the pending block, transforming it into a full block.
    async fn seal_pending_block(&mut self) {
        let start = Instant::now();

        // Apply fees of pending block
        let fee_updates = self.state.collect_fee(
            &self.pending_block.collected_fees,
            self.config.fee_account_id,
        );
        self.pending_block
            .account_updates
            .extend(fee_updates.into_iter());

        // TODO (ZKS-821): Currently the logic of this procedure is obscure and error-prone.
        // I've met multiple bugs trying to adapt it because it works at the same time with the "old"
        // pending block and "new" pending block. Actions "create block to be sealed" and "update pending block"
        // should be spearated.
        let current_block = self.pending_block.number;
        let next_unprocessed_priority_op = self.pending_block.unprocessed_priority_op_current;
        let mut pending_block = std::mem::replace(
            &mut self.pending_block,
            PendingBlock::new(
                current_block,
                next_unprocessed_priority_op,
                self.config.max_block_size(),
                H256::default(),
                system_time_timestamp(),
            ),
        );

        let mut block_transactions = pending_block.success_operations.clone(); // TODO (in this PR): Avoid cloning.
        block_transactions.extend(
            pending_block
                .failed_txs
                .iter()
                .cloned() // TODO (in this PR): Avoid cloning.
                .map(|tx| ExecutedOperations::Tx(Box::new(tx))),
        );

        let commit_gas_limit = pending_block.gas_counter.commit_gas_limit();
        let verify_gas_limit = pending_block.gas_counter.verify_gas_limit();

        let block = IncompleteBlock::new_from_available_block_sizes(
            pending_block.number,
            self.config.fee_account_id,
            block_transactions,
            (
                pending_block.unprocessed_priority_op_before,
                pending_block.unprocessed_priority_op_current,
            ),
            &self.config.available_block_chunk_sizes,
            commit_gas_limit,
            verify_gas_limit,
            pending_block.timestamp,
        );

        // Update the fields of the new pending block.
        *self.pending_block.number += 1;

        let block_metadata = BlockMetadata {
            fast_processing: pending_block.fast_processing_required,
        };

        for tx in &block.block_transactions {
            let labels = vec![
                ("stage", "seal_block".to_string()),
                ("name", tx.variance_name()),
                ("token", tx.token_id().to_string()),
            ];
            metrics::histogram!("process_tx", tx.elapsed(), &labels);
        }
        metrics::histogram!(
            "process_block",
            block.elapsed(),
            "stage" => "seal"
        );

        let block_commit_request = BlockCommitRequest {
            block,
            block_metadata,
            accounts_updated: pending_block.account_updates.clone(),
        };
        let applied_updates_request = pending_block.prepare_applied_updates_request();
        let root_hash_job = BlockRootHashJob {
            block: current_block,
            updates: pending_block.account_updates.clone(),
        };

        vlog::info!(
            "Creating full block: {}, operations: {}, chunks_left: {}, miniblock iterations: {}",
            *block_commit_request.block.block_number,
            block_commit_request.block.block_transactions.len(),
            pending_block.chunks_left,
            pending_block.pending_block_iteration
        );

        let commit_request =
            CommitRequest::SealIncompleteBlock((block_commit_request, applied_updates_request));
        self.tx_for_commitments
            .send(commit_request)
            .await
            .expect("committer receiver dropped");
        self.root_hash_queue.push(root_hash_job).await;

        metrics::histogram!("state_keeper.seal_pending_block", start.elapsed());
    }

    /// Stores intermediate representation of a pending block in the database,
    /// so the executed transactions are persisted and won't be lost.
    async fn store_pending_block(&mut self) {
        let start = Instant::now();

        let pending_block = self.pending_block.prepare_for_storing();
        let applied_updates_request = self.pending_block.prepare_applied_updates_request();

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
            last_block_number: self.pending_block.number - 1,
            unprocessed_priority_op: self.pending_block.unprocessed_priority_op_current,
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
