// External uses
// Workspace uses
use zksync_state::state::CollectedFee;
use zksync_types::{
    block::{ExecutedOperations, ExecutedTx},
    gas_counter::GasCounter,
    AccountUpdates, H256,
};
// Local uses

#[derive(Debug, Clone)]
pub(super) struct PendingBlock {
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
}

impl PendingBlock {
    pub(super) fn new(
        unprocessed_priority_op_before: u64,
        available_chunks_sizes: &[usize],
        previous_block_root_hash: H256,
        timestamp: u64,
    ) -> Self {
        // TransferOp chunks are subtracted to reserve space for last transfer.
        let chunks_left = *available_chunks_sizes
            .iter()
            .max()
            .expect("Expected at least one block chunks size");
        Self {
            success_operations: Vec::new(),
            failed_txs: Vec::new(),
            account_updates: Vec::new(),
            chunks_left,
            pending_op_block_index: 0,
            unprocessed_priority_op_before,
            pending_block_iteration: 0,
            gas_counter: GasCounter::new(),
            fast_processing_required: false,
            collected_fees: Vec::new(),
            stored_account_updates: 0,
            previous_block_root_hash,
            timestamp,
        }
    }
}
