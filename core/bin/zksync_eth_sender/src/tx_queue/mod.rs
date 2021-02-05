// Workspace imports
use zksync_types::{
    aggregated_operations::{AggregatedActionType, AggregatedOperation},
    BlockNumber,
};
// External uses
use anyhow::format_err;
// Local imports
use self::operation_queue::OperationQueue;

mod operation_queue;

pub type RawTxData = Vec<u8>;

/// Representation of the transaction data stored in the queue.
/// This structure contains only essential fields required for the `eth_sender`
/// to create an actual operation.
#[derive(Debug, Clone)]
pub struct TxData {
    /// Type of the operation.
    pub op_type: AggregatedActionType,
    /// Not signed raw tx payload.
    pub raw: RawTxData,
    /// Optional zkSync operation.
    pub operation: (i64, AggregatedOperation),
}

impl PartialEq for TxData {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl TxData {
    /// Creates a new `TxData` object with the associated zkSync operation.
    pub fn from_operation(operation: (i64, AggregatedOperation), raw: RawTxData) -> Self {
        Self {
            op_type: operation.1.get_action_type(),
            raw,
            operation,
        }
    }

    pub fn get_block_range(&self) -> (BlockNumber, BlockNumber) {
        self.operation.1.get_block_range()
    }
}

/// `TxQueueBuilder` is a structure aiming to simplify the process
/// of restoring of the `TxQueue` state after restart.
/// This structure allows to configure the sub-queues state (amount of processed
/// operations).
#[derive(Debug)]
pub struct TxQueueBuilder {
    max_pending_txs: usize,
    sent_pending_txs: usize,

    commit_operations_count: usize,
    verify_operations_count: usize,
    execute_operations_count: usize,
}

impl TxQueueBuilder {
    /// Initializes queue building process.
    pub fn new(max_pending_txs: usize) -> Self {
        Self {
            max_pending_txs,
            sent_pending_txs: 0,
            commit_operations_count: 0,
            verify_operations_count: 0,
            execute_operations_count: 0,
        }
    }

    /// Sets the amount of transactions sent to the Ethereum blockchain, but not confirmed yet.
    pub fn with_sent_pending_txs(self, sent_pending_txs: usize) -> Self {
        Self {
            sent_pending_txs,
            ..self
        }
    }

    /// Sets the amount of operations sent for the `commit` queue.
    pub fn with_commit_operations_count(self, commit_operations_count: usize) -> Self {
        Self {
            commit_operations_count,
            ..self
        }
    }

    /// Sets the amount of operations sent for the `verify` queue.
    pub fn with_verify_operations_count(self, verify_operations_count: usize) -> Self {
        Self {
            verify_operations_count,
            ..self
        }
    }

    /// Sets the amount of operations sent for the `execute` queue.
    pub fn with_execute_operations_count(self, execute_operations_count: usize) -> Self {
        Self {
            execute_operations_count,
            ..self
        }
    }

    /// Finishes the queue building process.
    pub fn build(self) -> TxQueue {
        TxQueue {
            max_pending_txs: self.max_pending_txs,
            sent_pending_txs: self.sent_pending_txs,

            commit_operations: OperationQueue::new(BlockNumber(
                self.commit_operations_count as u32,
            )),
            verify_operations: OperationQueue::new(BlockNumber(
                self.verify_operations_count as u32,
            )),
            execute_operations: OperationQueue::new(BlockNumber(
                self.execute_operations_count as u32,
            )),
        }
    }
}

/// Transaction queue combines the underlying operations queues and determines
/// the transaction sending policy. It chooses the next operation to send out of
/// these queues, using the following rules:
///
/// 1. If the amount of sent transactions is equal to the `MAX_PENDING_TXS` value,
///   no transaction is yielded until some of already sent ones are committed.
/// 2. Otherwise, transactions are yielded according to the following policy:
///   - If `execute` queue contains elements for some blocks, and `verify` operations
///     for corresponding blocks is committed, the `execute` operation is yielded.
///   - If `verify` queue contains elements for some blocks, and `commit` operations
///     for corresponding blocks is committed, the `verify` operation is yielded.
///   - Otherwise, if `commit` queue is not empty, a `commit` operation is yielded.
/// 3. If all the queues are empty, no operation is returned.
#[derive(Debug)]
pub struct TxQueue {
    max_pending_txs: usize,
    sent_pending_txs: usize,

    commit_operations: OperationQueue,
    verify_operations: OperationQueue,
    execute_operations: OperationQueue,
}

impl TxQueue {
    /// Adds the `commit` operation to the queue.
    pub fn add_commit_operation(&mut self, commit_operation: TxData) -> anyhow::Result<()> {
        self.commit_operations.push_back(commit_operation)?;

        vlog::info!(
            "Adding commit operation to the queue. \
            Sent pending txs count: {}, \
            max pending txs count: {}, \
            size of commit queue: {}",
            self.sent_pending_txs,
            self.max_pending_txs,
            self.commit_operations.len()
        );
        Ok(())
    }

    /// Adds the `verify` operation to the queue.
    pub fn add_verify_operation(&mut self, verify_operation: TxData) -> anyhow::Result<()> {
        self.verify_operations.push_back(verify_operation)?;

        vlog::info!(
            "Adding verify operation to the queue. \
            Sent pending txs count: {}, \
            max pending txs count: {}, \
            size of verify queue: {}",
            self.sent_pending_txs,
            self.max_pending_txs,
            self.verify_operations.len()
        );
        Ok(())
    }

    /// Adds the `execute` operation to the queue.
    pub fn add_execute_operation(&mut self, execute_operation: TxData) -> anyhow::Result<()> {
        self.execute_operations.push_back(execute_operation)?;

        vlog::info!(
            "Adding execute operation to the queue. \
            Sent pending txs count: {}, \
            max pending txs count: {}, \
            size of execute queue: {}",
            self.sent_pending_txs,
            self.max_pending_txs,
            self.execute_operations.len()
        );
        Ok(())
    }

    /// Returns a previously popped element to the front of the queue.
    pub fn return_popped(&mut self, element: TxData) -> anyhow::Result<()> {
        assert!(
            self.sent_pending_txs > 0,
            "No transactions are expected to be returned"
        );

        match &element.op_type {
            AggregatedActionType::CommitBlocks => {
                self.commit_operations.return_popped(element)?;
            }
            AggregatedActionType::PublishProofBlocksOnchain => {
                self.verify_operations.return_popped(element)?;
            }
            AggregatedActionType::ExecuteBlocks => {
                self.execute_operations.return_popped(element)?;
            }
            AggregatedActionType::CreateProofBlocks => {
                return Err(format_err!(
                    "Proof creation should never be sent to Ethereum"
                ));
            }
        }

        // We've incremented the counter when transaction was popped.
        // Now it's returned and counter should be decremented back.
        self.sent_pending_txs -= 1;
        Ok(())
    }

    /// Gets the next transaction to send, according to the transaction sending policy.
    /// For details, see the structure doc-comment.
    pub fn pop_front(&mut self) -> Option<TxData> {
        if self.sent_pending_txs >= self.max_pending_txs {
            return None;
        }

        // Get the next operation and increment the sent counter if needed.
        match self.get_next_operation() {
            Some(op) => {
                self.sent_pending_txs += 1;
                Some(op)
            }
            None => None,
        }
    }

    /// Obtains the next operation from the underlying queues.
    /// This method does not use/affect `sent_pending_tx` counter.
    fn get_next_operation(&mut self) -> Option<TxData> {
        // 1. Highest priority: execute operations.
        if let Some(next_execute_block) = self.execute_operations.get_next_last_block_number() {
            let current_verify_block = self.verify_operations.get_last_block_number();
            if *next_execute_block <= *current_verify_block {
                return Some(self.execute_operations.pop_front().unwrap());
            }
        }

        // 2. After execute operations we should process verify operation.
        if let Some(next_verify_block) = self.verify_operations.get_next_last_block_number() {
            let current_commit_block = self.commit_operations.get_last_block_number();
            if *next_verify_block <= *current_commit_block {
                return Some(self.verify_operations.pop_front().unwrap());
            }
        }

        // 3. Finally, check the commit queue.
        self.commit_operations.pop_front()
    }

    /// Notifies the queue about the transaction being confirmed on the Ethereum blockchain.
    /// Decrements the amount of transactions "in the fly".
    pub fn report_commitment(&mut self) {
        assert!(
            self.sent_pending_txs > 0,
            "No transactions are expected to be confirmed"
        );

        self.sent_pending_txs -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zksync_storage::test_data::{gen_unique_aggregated_operation, BLOCK_SIZE_CHUNKS};

    fn get_tx_data(
        operation_type: AggregatedActionType,
        block_number: BlockNumber,
        raw: RawTxData,
    ) -> TxData {
        let operation =
            gen_unique_aggregated_operation(block_number, operation_type, BLOCK_SIZE_CHUNKS);

        TxData::from_operation((*block_number as i64, operation), raw)
    }

    /// Checks the basic workflow of the queue including adding several operations
    /// and retrieving them later.
    #[test]
    fn basic_operations() {
        const MAX_IN_FLY: usize = 3;
        const COMMIT_MARK: u8 = 0;
        const VERIFY_MARK: u8 = 1;
        const EXECUTE_MARK: u8 = 2;

        let mut queue = TxQueueBuilder::new(MAX_IN_FLY).build();

        // Add 2 commit, 2 verify and 2 withdraw operations.
        queue
            .add_commit_operation(get_tx_data(
                AggregatedActionType::CommitBlocks,
                BlockNumber(1),
                vec![COMMIT_MARK, 0],
            ))
            .unwrap();
        queue
            .add_commit_operation(get_tx_data(
                AggregatedActionType::CommitBlocks,
                BlockNumber(2),
                vec![COMMIT_MARK, 1],
            ))
            .unwrap();
        queue
            .add_verify_operation(get_tx_data(
                AggregatedActionType::PublishProofBlocksOnchain,
                BlockNumber(1),
                vec![VERIFY_MARK, 0],
            ))
            .unwrap();
        queue
            .add_verify_operation(get_tx_data(
                AggregatedActionType::PublishProofBlocksOnchain,
                BlockNumber(2),
                vec![VERIFY_MARK, 1],
            ))
            .unwrap();
        queue
            .add_execute_operation(get_tx_data(
                AggregatedActionType::ExecuteBlocks,
                BlockNumber(1),
                vec![EXECUTE_MARK, 0],
            ))
            .unwrap();
        queue
            .add_execute_operation(get_tx_data(
                AggregatedActionType::ExecuteBlocks,
                BlockNumber(2),
                vec![EXECUTE_MARK, 1],
            ))
            .unwrap();

        // Retrieve the next {MAX_IN_FLY} operations.

        // The first operation should be `commit`, since we can't send `verify` before the commitment.
        let op_1 = queue.pop_front().unwrap();
        assert_eq!(op_1.raw, vec![COMMIT_MARK, 0]);

        // The second operation should be `verify`, since it has the highest priority.
        let op_2 = queue.pop_front().unwrap();
        assert_eq!(op_2.raw, vec![VERIFY_MARK, 0]);

        // The third operation should be `withdraw`, since it has higher priority than `commit`, and we can't
        // send the `verify` before the corresponding `commit` operation.
        let op_3 = queue.pop_front().unwrap();
        assert_eq!(op_3.raw, vec![EXECUTE_MARK, 0]);

        // After that we have {MAX_IN_FLY} operations, and `pop_front` should yield nothing.
        assert_eq!(queue.pop_front(), None);

        // Report that one operation is completed.
        queue.report_commitment();

        // Now we should obtain the next commit operation.
        let op_4 = queue.pop_front().unwrap();
        assert_eq!(op_4.raw, vec![COMMIT_MARK, 1]);

        // The limit should be met again, and nothing more should be yielded.
        assert_eq!(queue.pop_front(), None);

        // Report the remaining three operations as completed.
        assert_eq!(queue.sent_pending_txs, MAX_IN_FLY);
        for _ in 0..MAX_IN_FLY {
            queue.report_commitment();
        }
        assert_eq!(queue.sent_pending_txs, 0);

        // Pop remaining operations.
        let op_5 = queue.pop_front().unwrap();
        assert_eq!(op_5.raw, vec![VERIFY_MARK, 1]);

        let op_6 = queue.pop_front().unwrap();
        assert_eq!(op_6.raw, vec![EXECUTE_MARK, 1]);

        // Though the limit is not met (2 txs in fly, and limit is 3), there should be no txs in the queue.
        assert_eq!(queue.pop_front(), None);

        let pending_count = queue.sent_pending_txs;

        // Return the operation to the queue.
        queue.return_popped(op_6).unwrap();

        // Now, as we've returned tx to queue, pending count should be decremented.
        assert_eq!(queue.sent_pending_txs, pending_count - 1);

        let op_6 = queue.pop_front().unwrap();
        assert_eq!(op_6.raw, vec![EXECUTE_MARK, 1]);

        // We've popped the tx once again, now pending count should be increased.
        assert_eq!(queue.sent_pending_txs, pending_count);
    }

    #[test]
    #[should_panic(expected = "No transactions are expected to be returned")]
    fn return_popped_empty() {
        const MAX_IN_FLY: usize = 3;
        const COMMIT_MARK: u8 = 0;

        let mut queue = TxQueueBuilder::new(MAX_IN_FLY).build();

        queue
            .return_popped(get_tx_data(
                AggregatedActionType::CommitBlocks,
                BlockNumber(1),
                vec![COMMIT_MARK, 0],
            ))
            .unwrap();
    }
}
