// External uses
// Workspace uses
use zksync_state::state::CollectedFee;
use zksync_types::{
    block::{ExecutedOperations, ExecutedTx, PendingBlock as SendablePendingBlock},
    gas_counter::GasCounter,
    AccountUpdates, BlockNumber,
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
    pub(super) unprocessed_priority_op_current: u64,
    pub(super) pending_block_iteration: usize,
    pub(super) gas_counter: GasCounter,
    /// Option denoting if this block should be generated faster than usual.
    pub(super) fast_processing_required: bool,
    /// Fee should be applied only when sealing the block (because of corresponding logic in the circuit)
    pub(super) collected_fees: Vec<CollectedFee>,
    /// Number of stored account updates in the db (from `account_updates` field)
    pub(super) stored_account_updates: usize,
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
            unprocessed_priority_op_current: unprocessed_priority_op_before,
            pending_block_iteration: 0,
            gas_counter: GasCounter::new(),
            fast_processing_required: false,
            collected_fees: Vec::new(),
            stored_account_updates: 0,
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
        // `>=` in condition since iterations start with 0.
        self.chunks_left == 0 || self.pending_block_iteration >= miniblock_iterations
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

        if exec_result.is_priority() {
            self.unprocessed_priority_op_current += 1;
        }

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

#[cfg(test)]
mod tests {
    use chrono::prelude::*;
    use zksync_types::{
        AccountId, AccountUpdate, Address, Nonce, SignedZkSyncTx, TokenId, Transfer, ZkSyncTx,
    };

    use super::*;

    const STARTING_BLOCK: BlockNumber = BlockNumber(1);
    const CHUNKS_PER_BLOCK: usize = 100;
    const MAX_ITERATIONS: usize = 2;

    fn pending_block() -> PendingBlock {
        // Fields that aren't interesting in the testing context.
        let unprocessed_priority_op_before = 0;
        let timestamp = 0;

        PendingBlock::new(
            STARTING_BLOCK,
            unprocessed_priority_op_before,
            CHUNKS_PER_BLOCK,
            timestamp,
        )
    }

    /// Creates a mock `ExecutedOperations` object.
    /// Actual operation doesn't matter since pending block does not interact with operations, it just stores it.
    fn mock_executed_op() -> ExecutedOperations {
        let tx = ZkSyncTx::Transfer(Box::new(Transfer::new(
            AccountId(0),
            Address::default(),
            Address::default(),
            TokenId(0),
            100u64.into(),
            100u64.into(),
            Nonce(0),
            Default::default(),
            None,
        )));
        let signed_tx = SignedZkSyncTx {
            tx,
            eth_sign_data: None,
            created_at: Utc::now(),
        };

        ExecutedOperations::Tx(Box::new(ExecutedTx {
            signed_tx,
            success: false,
            op: None,
            fail_reason: Some("Mock".to_string()),
            block_index: None,
            created_at: Utc.ymd(2021, 12, 9).and_hms(12, 26, 11),
            batch_id: None,
        }))
    }

    /// Creates all the fields to call `add_successfull_execution`.
    fn prepare_successful_execution() -> (
        usize,
        AccountUpdates,
        Option<CollectedFee>,
        ExecutedOperations,
    ) {
        let chunks = 2;
        let updates = vec![(
            AccountId(0),
            AccountUpdate::Create {
                address: Address::repeat_byte(0x7a),
                nonce: Nonce(0),
            },
        )];
        let fee = Some(CollectedFee {
            token: TokenId(0),
            amount: 100u64.into(),
        });
        let exec_result = mock_executed_op();
        (chunks, updates, fee, exec_result)
    }

    #[test]
    fn basic_properties() {
        let mut pending_block = pending_block();

        // Checks for empty block.
        assert_eq!(
            pending_block.pending_block_iteration, 0,
            "Should start on 0 iteration"
        );
        assert_eq!(
            pending_block.chunks_left, CHUNKS_PER_BLOCK,
            "No chunks should be used at start"
        );

        // Methods testing on the empty block.
        assert!(pending_block.is_empty(), "Block should be empty");
        assert!(
            !pending_block.should_seal(MAX_ITERATIONS),
            "Should no seal empty block with no enough iterations"
        );

        pending_block.increment_iteration();
        assert_eq!(
            pending_block.pending_block_iteration, 0,
            "Iteration should not be incremented for an empty block"
        );

        // Add some operation to the pending block.
        let (chunks, updates, fee, exec_result) = prepare_successful_execution();
        pending_block.add_successful_execution(chunks, updates, fee, exec_result);

        // Check pending block state after execution.
        assert_eq!(
            pending_block.chunks_left,
            CHUNKS_PER_BLOCK - chunks,
            "Chunks were not subtracted"
        );
        assert!(!pending_block.is_empty(), "Block is not empty anymore");

        pending_block.increment_iteration();
        assert_eq!(
            pending_block.pending_block_iteration, 1,
            "Iteration should be incremented"
        );

        assert!(
            !pending_block.should_seal(MAX_ITERATIONS),
            "Block should not be sealed after 1 iteration"
        );

        pending_block.increment_iteration();
        assert_eq!(
            pending_block.pending_block_iteration, 2,
            "Iteration should be incremented"
        );

        assert!(
            pending_block.should_seal(MAX_ITERATIONS),
            "Block should be sealed after 2 iteration"
        );

        // Call for "preparing" methods which should update internal state.
        let sendable = pending_block.prepare_for_storing();
        assert_eq!(sendable.number, STARTING_BLOCK);
        assert_eq!(
            sendable.success_operations.len(),
            pending_block.success_operations.len(),
            "Success operations should be included into sendable pending block"
        );
        assert_eq!(
            pending_block.success_txs_pending_len,
            pending_block.success_operations.len(),
            "Pending block should offset stored success txs counter"
        );

        let applied_updates = pending_block.prepare_applied_updates_request();
        assert_eq!(applied_updates.first_update_order_id, 0);
        assert_eq!(
            applied_updates.account_updates.len(),
            pending_block.account_updates.len(),
            "Account updates should be included into applied updates request"
        );
        assert_eq!(
            pending_block.stored_account_updates,
            pending_block.account_updates.len(),
            "Pending block should offset stored applied updates counter"
        );

        // Calling these method again with no changes to the pending block should contain no updates.
        let sendable = pending_block.prepare_for_storing();
        assert_eq!(sendable.number, STARTING_BLOCK);
        assert_eq!(
            sendable.success_operations.len(),
            0,
            "There were no new operations"
        );

        let applied_updates = pending_block.prepare_applied_updates_request();
        assert_eq!(
            applied_updates.first_update_order_id, pending_block.stored_account_updates,
            "Updates should start from the point where we stopped last time"
        );
        assert_eq!(
            applied_updates.account_updates.len(),
            0,
            "There were no new updates"
        );

        // Now apply one more success operation and it should be included into the next sendable block
        // and applied updates request.
        let chunks_before = pending_block.chunks_left;
        let (chunks, updates, fee, exec_result) = prepare_successful_execution();
        pending_block.add_successful_execution(chunks, updates.clone(), fee, exec_result);

        assert_eq!(
            pending_block.chunks_left,
            chunks_before - chunks,
            "Chunks were not subtracted for 2nd operation"
        );

        let sendable = pending_block.prepare_for_storing();
        assert_eq!(sendable.number, STARTING_BLOCK);
        assert_eq!(
            sendable.success_operations.len(),
            1,
            "There was 1 new operation"
        );

        let next_update_before = pending_block.stored_account_updates;
        let applied_updates = pending_block.prepare_applied_updates_request();
        assert_eq!(
            applied_updates.first_update_order_id, next_update_before,
            "Updates should start from the point where we stopped last time"
        );
        assert_eq!(
            applied_updates.account_updates.len(),
            updates.len(),
            "There were new updates"
        );
        assert_eq!(
            pending_block.stored_account_updates,
            pending_block.account_updates.len(),
        )
    }
}
