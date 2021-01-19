// Built-in imports
use std::collections::VecDeque;
// Workspace imports
use zksync_types::ethereum::OperationType;
// Local imports
use crate::tx_queue::TxData;

/// Counter queue is basically a queue which
/// tracks the amount of the processed elements.
/// Main feature is that it counts not the number of popped elements,
/// but the number of popped element groups.
#[derive(Debug)]
pub struct WithdrawalsCounterQueue {
    pub(super) elements: VecDeque<(usize, TxData)>,
    counter: usize,
}

impl Default for WithdrawalsCounterQueue {
    fn default() -> Self {
        Self {
            counter: 0,
            elements: VecDeque::new(),
        }
    }
}

impl WithdrawalsCounterQueue {
    /// Creates a new empty counter queue with the custom number of processed elements.
    pub fn new(counter: usize) -> Self {
        Self {
            counter,
            ..Default::default()
        }
    }

    /// Returns a previously popped element to the front of the queue.
    pub fn return_popped(&mut self, element: TxData) {
        assert_eq!(
            element.op_type,
            OperationType::Withdraw,
            "WithdrawalsCounterQueue should only be used for withdrawals"
        );

        if let Some((id, front_element)) = self.elements.front_mut() {
            if front_element.block() == element.block() {
                *id += 1;

                return;
            }
        }

        self.elements.push_front((1, element));
        self.counter -= 1;
    }

    /// Inserts an the number of operations and the operation itself to the end of the queue.
    pub fn push_back(&mut self, count: usize, withdraw_operation: TxData) {
        assert!(count >= 1, "number of operations must be positive");

        self.elements.push_back((count, withdraw_operation));
    }

    /// Attempts to take the next element from the queue. Returns `None`
    /// if the queue is empty.
    ///
    /// If the last element from the queue is associated with the number of operations greater than 1, then it does not change the counter.
    pub fn pop_front(&mut self) -> Option<TxData> {
        match self.elements.pop_front() {
            Some((count, element)) => {
                if count == 1 {
                    self.counter += 1;
                } else {
                    self.elements.push_front((count - 1, element.clone()));
                }

                Some(element)
            }
            None => None,
        }
    }

    /// Returns the value of the counter.
    pub fn get_count(&self) -> usize {
        self.counter
    }

    /// Returns the current size of the withdrawals queue.
    pub fn len(&self) -> usize {
        self.elements.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zksync_types::{block::Block, Action, BlockNumber, Operation};

    fn get_withdraw_op(block_number: BlockNumber) -> TxData {
        let block = Block::new_from_available_block_sizes(
            block_number,
            Default::default(),
            0,
            vec![],
            (0, 0),
            &[0],
            1_000_000.into(),
            1_500_000.into(),
        );
        let operation = Operation {
            id: None,
            action: Action::Commit,
            block,
        };

        TxData::from_operation(OperationType::Withdraw, operation, Default::default())
    }

    /// Checks the main operations of the queue: `push_back`, `pop_front` and `get_count`.
    #[test]
    fn basic_operations() {
        let mut queue = WithdrawalsCounterQueue::new(0);

        // Check that by default the current count is 0.
        assert_eq!(queue.get_count(), 0);

        // Insert the next element and obtain it.
        queue.push_back(1, get_withdraw_op(1));
        // Inserting the element should NOT update the counter.
        assert_eq!(queue.get_count(), 0);
        assert_eq!(queue.pop_front().unwrap(), get_withdraw_op(1));
        // After taking the element, the counter should be updated.
        assert_eq!(queue.get_count(), 1);

        // Perform the same check again and check that overall counter will become 2.
        queue.push_back(2, get_withdraw_op(2));
        assert_eq!(queue.get_count(), 1);
        assert_eq!(queue.pop_front().unwrap(), get_withdraw_op(2));
        assert_eq!(queue.get_count(), 1);
        assert_eq!(queue.pop_front().unwrap(), get_withdraw_op(2));
        assert_eq!(queue.get_count(), 2);

        // Now attempt take no value, and check that counter is not increased.
        assert_eq!(queue.pop_front(), None);
        assert_eq!(queue.get_count(), 2);

        // Return the popped element back.
        queue.return_popped(get_withdraw_op(2));
        assert_eq!(queue.get_count(), 1);

        assert_eq!(queue.pop_front().unwrap(), get_withdraw_op(2));
        assert_eq!(queue.get_count(), 2);

        // If popped element have the same block number as front element
        // then return the popped element should NOT change the counter.
        queue.push_back(3, get_withdraw_op(3));
        assert_eq!(queue.get_count(), 2);

        assert_eq!(queue.pop_front().unwrap(), get_withdraw_op(3));
        assert_eq!(queue.get_count(), 2);

        assert_eq!(queue.pop_front().unwrap(), get_withdraw_op(3));
        assert_eq!(queue.get_count(), 2);

        assert_eq!(queue.pop_front().unwrap(), get_withdraw_op(3));
        assert_eq!(queue.get_count(), 3);

        queue.return_popped(get_withdraw_op(3));
        queue.return_popped(get_withdraw_op(3));
        assert_eq!(queue.get_count(), 2);

        assert_eq!(queue.pop_front().unwrap(), get_withdraw_op(3));
        assert_eq!(queue.pop_front().unwrap(), get_withdraw_op(3));
        assert_eq!(queue.get_count(), 3);
    }
}
