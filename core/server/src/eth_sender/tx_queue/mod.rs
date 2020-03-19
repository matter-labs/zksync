// TODO: Remove me
#![allow(dead_code)]

use self::{counter_queue::CounterQueue, sparse_queue::SparseQueue};

mod counter_queue;
mod sparse_queue;

pub type RawTxData = Vec<u8>;

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
    withdraw_operations_count: usize,
}

impl TxQueueBuilder {
    /// Initializes queue building process.
    pub fn new(max_pending_txs: usize) -> Self {
        Self {
            max_pending_txs,
            sent_pending_txs: 0,
            commit_operations_count: 0,
            verify_operations_count: 0,
            withdraw_operations_count: 0,
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

    /// Sets the amount of operations sent for the `pending` queue.
    pub fn with_verify_operations_count(self, verify_operations_count: usize) -> Self {
        Self {
            verify_operations_count,
            ..self
        }
    }

    /// Sets the amount of operations sent for the `withdraw` queue.
    pub fn with_withdraw_operations_count(self, withdraw_operations_count: usize) -> Self {
        Self {
            withdraw_operations_count,
            ..self
        }
    }

    /// Finishes the queue building process.
    pub fn build(self) -> TxQueue {
        TxQueue {
            max_pending_txs: self.max_pending_txs,
            sent_pending_txs: self.sent_pending_txs,

            commit_operations: CounterQueue::new_with_count(self.commit_operations_count),
            verify_operations: SparseQueue::new_from(self.verify_operations_count),
            withdraw_operations: CounterQueue::new_with_count(self.withdraw_operations_count),
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
///   - If `verify` queue contains elements, and `commit` operation with corresponding
///     ID is committed, the `verify` operation is yielded (meaning that `verify` operations
///     are prioritized unless the amount of sent `commit` and `verify` operations is equal:
///     if so, we should send the `commit` operation first).
///   - Otherwise, if `withdraw` queue contains elements, a `withdraw` operation is yielded.
///   - Otherwise, if `commit` queue is not empty, a `commit` operation is yielded.
/// 3. If all the queues are empty, no operation is returned.
#[derive(Debug)]
pub struct TxQueue {
    max_pending_txs: usize,
    sent_pending_txs: usize,

    commit_operations: CounterQueue<RawTxData>,
    verify_operations: SparseQueue<RawTxData>,
    withdraw_operations: CounterQueue<RawTxData>,
}

impl TxQueue {
    /// Creates a new empty transactions queue.
    pub fn new(max_pending_txs: usize) -> Self {
        Self {
            max_pending_txs,
            sent_pending_txs: 0,

            commit_operations: CounterQueue::new(),
            verify_operations: SparseQueue::new(),
            withdraw_operations: CounterQueue::new(),
        }
    }

    /// Creates a new empty transactions queue with the custom expected next ID
    /// for the `Verify` operations queue.
    /// This method is used to restore the state of the queue.
    pub fn new_from(max_pending_txs: usize, idx: usize) -> Self {
        Self {
            verify_operations: SparseQueue::new_from(idx),
            ..Self::new(max_pending_txs)
        }
    }

    /// Adds the `commit` operation to the queue.
    pub fn add_commit_operation(&mut self, commit_operation: RawTxData) {
        self.commit_operations.push_back(commit_operation);
    }

    /// Adds the `verify` operation to the queue.
    pub fn add_verify_operation(&mut self, block_idx: usize, verify_operation: RawTxData) {
        self.verify_operations.insert(block_idx, verify_operation);
    }

    /// Adds the `withdraw` operation to the queue.
    pub fn add_withdraw_operation(&mut self, withdraw_operation: RawTxData) {
        self.withdraw_operations.push_back(withdraw_operation);
    }

    /// Gets the next transaction to send, according to the transaction sending policy.
    /// For details, see the structure doc-comment.
    pub fn pop_front(&mut self) -> Option<RawTxData> {
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
    fn get_next_operation(&mut self) -> Option<RawTxData> {
        // 1. Highest priority: verify operations.

        // If we've committed a corresponding `Commit` operation, and
        // there is a pending `verify` operation, chose it.
        let next_verify_op_id = self.verify_operations.next_id();
        if next_verify_op_id < self.commit_operations.get_count()
            && self.verify_operations.has_next()
        {
            return Some(self.verify_operations.pop_front().unwrap());
        }

        // 2. After verify operations we should process withdraw operation.

        // We don't want to be ahead of the last verify operation.
        if self.withdraw_operations.get_count() < next_verify_op_id {
            if let Some(withdraw_operation) = self.withdraw_operations.pop_front() {
                return Some(withdraw_operation);
            }
        }

        // 3. Finally, check the commit queue.

        if let Some(commit_operation) = self.commit_operations.pop_front() {
            return Some(commit_operation);
        }

        // 4. There are no operations to process, return `None`.

        None
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

    /// Checks the basic workflow of the queue including adding several operations
    /// and retrieving them later.
    #[test]
    fn basic_operations() {
        const MAX_IN_FLY: usize = 3;
        const COMMIT_MARK: u8 = 0;
        const VERIFY_MARK: u8 = 1;
        const WITHDRAW_MARK: u8 = 2;

        let mut queue = TxQueue::new(MAX_IN_FLY);

        // Add 2 commit, 2 verify and 2 withdraw operations.
        queue.add_commit_operation(vec![COMMIT_MARK, 0]);
        queue.add_commit_operation(vec![COMMIT_MARK, 1]);
        queue.add_verify_operation(0, vec![VERIFY_MARK, 0]);
        queue.add_verify_operation(1, vec![VERIFY_MARK, 1]);
        queue.add_withdraw_operation(vec![WITHDRAW_MARK, 0]);
        queue.add_withdraw_operation(vec![WITHDRAW_MARK, 1]);

        // Retrieve the next {MAX_IN_FLY} operations.

        // The first operation should be `commit`, since we can't send `verify` before the commitment.
        let op_1 = queue.pop_front().unwrap();
        assert_eq!(op_1, vec![COMMIT_MARK, 0]);

        // The second operation should be `verify`, since it has the highest priority.
        let op_2 = queue.pop_front().unwrap();
        assert_eq!(op_2, vec![VERIFY_MARK, 0]);

        // The third operation should be `withdraw`, since it has higher priority than `commit`, and we can't
        // send the `verify` before the corresponding `commit` operation.
        let op_3 = queue.pop_front().unwrap();
        assert_eq!(op_3, vec![WITHDRAW_MARK, 0]);

        // After that we have {MAX_IN_FLY} operations, and `pop_front` should yield nothing.
        assert_eq!(queue.pop_front(), None);

        // Report that one operation is completed.
        queue.report_commitment();

        // Now we should obtain the next commit operation.
        let op_4 = queue.pop_front().unwrap();
        assert_eq!(op_4, vec![COMMIT_MARK, 1]);

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
        assert_eq!(op_5, vec![VERIFY_MARK, 1]);

        let op_6 = queue.pop_front().unwrap();
        assert_eq!(op_6, vec![WITHDRAW_MARK, 1]);

        // Though the limit is not met (2 txs in fly, and limit is 3), there should be no txs in the queue.
        assert_eq!(queue.pop_front(), None);
    }
}
