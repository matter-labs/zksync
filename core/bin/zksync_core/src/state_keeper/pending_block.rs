// External uses
// Workspace uses
use zksync_state::state::CollectedFee;
use zksync_types::{
    block::{ExecutedOperations, ExecutedTx, PendingBlock as SendablePendingBlock},
    gas_counter::GasCounter,
    AccountUpdates, BlockNumber, H256,
};

use crate::committer::AppliedUpdatesRequest;
// Local uses

#[derive(Debug, Clone)]
pub(super) struct PendingBlock {
    pub(super) number: BlockNumber,

    pub(super) success_operations: Vec<ExecutedOperations>,
    pub(super) failed_txs: Vec<ExecutedTx>,
    pub(super) account_updates: AccountUpdates,
    pub(super) chunks_left: usize,
    pub(super) pending_op_block_index: u32,
    pub(super) unprocessed_priority_op_before: u64,
    pub(super) pending_block_iteration: usize,
    pub(super) gas_counter: GasCounter,
    /// Option denoting if this block should be generated faster than usual.
    pub(super) fast_processing_required: bool,
    /// Fee should be applied only when sealing the block (because of corresponding logic in the circuit)
    pub(super) collected_fees: Vec<CollectedFee>,
    /// Number of stored account updates in the db (from `account_updates` field)
    pub(super) stored_account_updates: usize,
    pub(super) previous_block_root_hash: H256,
    pub(super) timestamp: u64,

    // Two fields below are for optimization: we don't want to overwrite all the block contents over and over.
    // With these fields we'll be able save the diff between two pending block states only.
    /// Amount of succeeded transactions in the pending block at the last pending block synchronization step.
    success_txs_pending_len: usize,
    /// Amount of failed transactions in the pending block at the last pending block synchronization step.
    failed_txs_pending_len: usize,
}

impl PendingBlock {
    pub(super) fn new(
        number: BlockNumber,
        unprocessed_priority_op_before: u64,
        max_block_size: usize,
        previous_block_root_hash: H256,
        timestamp: u64,
    ) -> Self {
        Self {
            number,
            success_operations: Vec::new(),
            failed_txs: Vec::new(),
            account_updates: Vec::new(),
            chunks_left: max_block_size,
            pending_op_block_index: 0,
            unprocessed_priority_op_before,
            pending_block_iteration: 0,
            gas_counter: GasCounter::new(),
            fast_processing_required: false,
            collected_fees: Vec::new(),
            stored_account_updates: 0,
            previous_block_root_hash,
            timestamp,

            success_txs_pending_len: 0,
            failed_txs_pending_len: 0,
        }
    }

    pub(super) fn increment_iteration(&mut self) {
        if !self.success_operations.is_empty() {
            self.pending_block_iteration += 1;
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.failed_txs.is_empty() && self.success_operations.is_empty()
    }

    pub(super) fn should_seal(&self, miniblock_iterations: usize) -> bool {
        self.chunks_left == 0 || self.pending_block_iteration > miniblock_iterations
    }

    pub(super) fn add_successful_execution(
        &mut self,
        chunks_used: usize,
        mut updates: AccountUpdates,
        fee: Option<CollectedFee>,
        exec_result: ExecutedOperations,
    ) {
        // If case of underflow we have to provide more context to ease the debugging.
        self.chunks_left = self
            .chunks_left
            .checked_sub(chunks_used)
            .unwrap_or_else(|| {
                panic!(
                    "Attempt to subract chunks with underflow. \n \
                 chunks_used: {}, executed op: {:?} \n \
                 current pending block state: {:?}",
                    chunks_used, exec_result, self,
                );
            });
        self.account_updates.append(&mut updates);
        if let Some(fee) = fee {
            self.collected_fees.push(fee);
        }
        self.pending_op_block_index += 1;
        self.success_operations.push(exec_result);
    }

    /// Creates `SendablePendingBlock needed to store pending block.
    /// Updates internal counters for already stored operations.
    pub(super) fn prepare_for_storing(&mut self) -> SendablePendingBlock {
        // We want include only the newly appeared transactions, since the older ones are already persisted in the
        // database.
        // This is a required optimization, since otherwise time to process the pending block may grow without any
        // limits if we'll be spammed by incorrect transactions (we don't have a limit for an amount of rejected
        // transactions in the block).
        let new_success_operations =
            self.success_operations[self.success_txs_pending_len..].to_vec();
        let new_failed_operations = self.failed_txs[self.failed_txs_pending_len..].to_vec();

        self.success_txs_pending_len = self.success_operations.len();
        self.failed_txs_pending_len = self.failed_txs.len();

        // Create a pending block object to send.
        // Note that failed operations are not included, as per any operation failure
        // the full block is created immediately.
        SendablePendingBlock {
            number: self.number,
            chunks_left: self.chunks_left,
            unprocessed_priority_op_before: self.unprocessed_priority_op_before,
            pending_block_iteration: self.pending_block_iteration,
            success_operations: new_success_operations,
            failed_txs: new_failed_operations,
            previous_block_root_hash: self.previous_block_root_hash,
            timestamp: self.timestamp,
        }
    }

    /// Creates `AppliedUpdatesRequest` needed to store pending or full block.
    /// Updates internal counters for already stored operations.
    pub(super) fn prepare_applied_updates_request(&mut self) -> AppliedUpdatesRequest {
        let first_update_order_id = self.stored_account_updates;
        let account_updates = self.account_updates[first_update_order_id..].to_vec();
        let applied_updates_request = AppliedUpdatesRequest {
            account_updates,
            first_update_order_id,
        };
        self.stored_account_updates = self.account_updates.len();

        applied_updates_request
    }
}
