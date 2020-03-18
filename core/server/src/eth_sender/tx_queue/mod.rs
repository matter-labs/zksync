// TODO: Remove me
#![allow(dead_code)]

use eth_client::SignedCallResult;

use self::{counter_queue::CounterQueue, sparse_queue::SparseQueue};

mod counter_queue;
mod sparse_queue;

#[derive(Debug)]
pub struct TxQueueBuilder {
    max_pending_txs: usize,
    sent_pending_txs: usize,

    commit_operations_count: usize,
    verify_operations_count: usize,
    withdraw_operations_count: usize,
}

impl TxQueueBuilder {
    pub fn new(max_pending_txs: usize) -> Self {
        Self {
            max_pending_txs,
            sent_pending_txs: 0,
            commit_operations_count: 0,
            verify_operations_count: 0,
            withdraw_operations_count: 0,
        }
    }

    pub fn with_sent_pending_txs(self, sent_pending_txs: usize) -> Self {
        Self {
            sent_pending_txs,
            ..self
        }
    }

    pub fn with_commit_operations_count(self, commit_operations_count: usize) -> Self {
        Self {
            commit_operations_count,
            ..self
        }
    }

    pub fn with_verify_operations_count(self, verify_operations_count: usize) -> Self {
        Self {
            verify_operations_count,
            ..self
        }
    }

    pub fn with_withdraw_operations_count(self, withdraw_operations_count: usize) -> Self {
        Self {
            withdraw_operations_count,
            ..self
        }
    }

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

    // TODO: SignedCallResult isn't appropriate, since it means an assigned nonce. We don't want
    // to assign nonce until the actual tx send.
    commit_operations: CounterQueue<SignedCallResult>,
    verify_operations: SparseQueue<SignedCallResult>,
    withdraw_operations: CounterQueue<SignedCallResult>,
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
    pub fn add_commit_operation(&mut self, commit_operation: SignedCallResult) {
        self.commit_operations.push_back(commit_operation);
    }

    /// Adds the `verify` operation to the queue.
    pub fn add_verify_operation(&mut self, block_idx: usize, verify_operation: SignedCallResult) {
        self.verify_operations.insert(block_idx, verify_operation);
    }

    /// Adds the `withdraw` operation to the queue.
    pub fn add_withdraw_operation(&mut self, withdraw_operation: SignedCallResult) {
        self.withdraw_operations.push_back(withdraw_operation);
    }

    /// Gets the next transaction to send, according to the transaction sending policy.
    /// For details, see the structure doc-comment.
    pub fn pop_front(&mut self) -> Option<SignedCallResult> {
        if self.sent_pending_txs >= self.max_pending_txs {
            return None;
        }

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

        if let Some(withdraw_operation) = self.withdraw_operations.pop_front() {
            return Some(withdraw_operation);
        }

        // 3. Finally, check the commit queue.

        if let Some(commit_operation) = self.commit_operations.pop_front() {
            return Some(commit_operation);
        }

        // 4. There are no operations to process, return `None`.

        None
    }
}
