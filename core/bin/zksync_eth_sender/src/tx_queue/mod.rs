// Workspace imports
use zksync_types::{ethereum::OperationType, BlockNumber, Operation};
// Local imports
use self::{counter_queue::CounterQueue, sparse_queue::SparseQueue};
use zksync_types::aggregated_operations::{AggregatedActionType, AggregatedOperation};

mod counter_queue;
mod sparse_queue;

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
}

/// `TxQueueBuilder` is a structure aiming to simplify the process
/// of restoring of the `TxQueue` state after restart.
/// This structure allows to configure the sub-queues state (amount of processed
/// operations).
#[derive(Debug)]
pub struct TxQueueBuilder {
    max_pending_txs: usize,
    sent_pending_txs: usize,

    aggregated_ops_count: usize,
}

impl TxQueueBuilder {
    /// Initializes queue building process.
    pub fn new(max_pending_txs: usize) -> Self {
        Self {
            max_pending_txs,
            sent_pending_txs: 0,
            aggregated_ops_count: 0,
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
    pub fn with_aggregated_ops_count(self, aggregated_ops_count: usize) -> Self {
        Self {
            aggregated_ops_count,
            ..self
        }
    }

    /// Finishes the queue building process.
    pub fn build(self) -> TxQueue {
        TxQueue {
            max_pending_txs: self.max_pending_txs,
            sent_pending_txs: self.sent_pending_txs,

            aggregated_operations: CounterQueue::new(self.aggregated_ops_count),
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

    aggregated_operations: CounterQueue<TxData>,
}

impl TxQueue {
    pub fn add_aggregate_operation(&mut self, aggregate_operation: TxData) {
        if let Some(TxData {
            operation: (id, _), ..
        }) = self.aggregated_operations.back()
        {
            if *id >= aggregate_operation.operation.0 {
                return;
            }
        }

        self.aggregated_operations.push_back(aggregate_operation);

        log::info!(
            "Adding operation to the queue. \
            Sent pending txs count: {}, \
            max pending txs count: {}, \
            size of op queue: {}",
            self.sent_pending_txs,
            self.max_pending_txs,
            self.aggregated_operations.len()
        );
    }

    /// Returns a previously popped element to the front of the queue.
    pub fn return_popped(&mut self, element: TxData) {
        assert!(
            self.sent_pending_txs > 0,
            "No transactions are expected to be returned"
        );

        self.aggregated_operations.return_popped(element);

        // We've incremented the counter when transaction was popped.
        // Now it's returned and counter should be decremented back.
        self.sent_pending_txs -= 1;
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
        // 1. Highest priority: verify operations.
        self.aggregated_operations.pop_front()
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
        todo!()
        // const MAX_IN_FLY: usize = 3;
        // const COMMIT_MARK: u8 = 0;
        // const VERIFY_MARK: u8 = 1;
        // const WITHDRAW_MARK: u8 = 2;
        //
        // let mut queue = TxQueueBuilder::new(MAX_IN_FLY).build();
        //
        // // Add 2 commit, 2 verify and 2 withdraw operations.
        // queue.add_commit_operation(TxData::from_raw(
        //     OperationType::Commit,
        //     vec![COMMIT_MARK, 0],
        // ));
        // queue.add_commit_operation(TxData::from_raw(
        //     OperationType::Commit,
        //     vec![COMMIT_MARK, 1],
        // ));
        // queue.add_verify_operation(
        //     1,
        //     TxData::from_raw(OperationType::Verify, vec![VERIFY_MARK, 0]),
        // );
        // queue.add_verify_operation(
        //     2,
        //     TxData::from_raw(OperationType::Verify, vec![VERIFY_MARK, 1]),
        // );
        // queue.add_withdraw_operation(TxData::from_raw(
        //     OperationType::Withdraw,
        //     vec![WITHDRAW_MARK, 0],
        // ));
        // queue.add_withdraw_operation(TxData::from_raw(
        //     OperationType::Withdraw,
        //     vec![WITHDRAW_MARK, 1],
        // ));
        //
        // // Retrieve the next {MAX_IN_FLY} operations.
        //
        // // The first operation should be `commit`, since we can't send `verify` before the commitment.
        // let op_1 = queue.pop_front().unwrap();
        // assert_eq!(op_1.raw, vec![COMMIT_MARK, 0]);
        //
        // // The second operation should be `verify`, since it has the highest priority.
        // let op_2 = queue.pop_front().unwrap();
        // assert_eq!(op_2.raw, vec![VERIFY_MARK, 0]);
        //
        // // The third operation should be `withdraw`, since it has higher priority than `commit`, and we can't
        // // send the `verify` before the corresponding `commit` operation.
        // let op_3 = queue.pop_front().unwrap();
        // assert_eq!(op_3.raw, vec![WITHDRAW_MARK, 0]);
        //
        // // After that we have {MAX_IN_FLY} operations, and `pop_front` should yield nothing.
        // assert_eq!(queue.pop_front(), None);
        //
        // // Report that one operation is completed.
        // queue.report_commitment();
        //
        // // Now we should obtain the next commit operation.
        // let op_4 = queue.pop_front().unwrap();
        // assert_eq!(op_4.raw, vec![COMMIT_MARK, 1]);
        //
        // // The limit should be met again, and nothing more should be yielded.
        // assert_eq!(queue.pop_front(), None);
        //
        // // Report the remaining three operations as completed.
        // assert_eq!(queue.sent_pending_txs, MAX_IN_FLY);
        // for _ in 0..MAX_IN_FLY {
        //     queue.report_commitment();
        // }
        // assert_eq!(queue.sent_pending_txs, 0);
        //
        // // Pop remaining operations.
        // let op_5 = queue.pop_front().unwrap();
        // assert_eq!(op_5.raw, vec![VERIFY_MARK, 1]);
        //
        // let op_6 = queue.pop_front().unwrap();
        // assert_eq!(op_6.raw, vec![WITHDRAW_MARK, 1]);
        //
        // // Though the limit is not met (2 txs in fly, and limit is 3), there should be no txs in the queue.
        // assert_eq!(queue.pop_front(), None);
        //
        // let pending_count = queue.sent_pending_txs;
        //
        // // Return the operation to the queue.
        // queue.return_popped(op_6);
        //
        // // Now, as we've returned tx to queue, pending count should be decremented.
        // assert_eq!(queue.sent_pending_txs, pending_count - 1);
        //
        // let op_6 = queue.pop_front().unwrap();
        // assert_eq!(op_6.raw, vec![WITHDRAW_MARK, 1]);
        //
        // // We've popped the tx once again, now pending count should be increased.
        // assert_eq!(queue.sent_pending_txs, pending_count);
    }

    #[test]
    #[should_panic(expected = "No transactions are expected to be returned")]
    fn return_popped_empty() {
        todo!()
        // const MAX_IN_FLY: usize = 3;
        // const COMMIT_MARK: u8 = 0;
        //
        // let mut queue = TxQueueBuilder::new(MAX_IN_FLY).build();
        //
        // queue.return_popped(TxData::from_raw(
        //     OperationType::Commit,
        //     vec![COMMIT_MARK, 0],
        // ));
    }
}
