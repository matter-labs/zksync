// Built-in imports
use std::collections::VecDeque;
// External uses
use anyhow::format_err;
// Local imports
use crate::tx_queue::TxData;
use zksync_types::BlockNumber;

/// Ethereum Transaction queue is basically a queue which
/// contains `TxData` and tracks the last popped block number.
///
/// Must receive operations in ascending order of affected blocks.
#[derive(Debug)]
pub struct OperationQueue {
    pub(super) elements: VecDeque<TxData>,
    last_block_number: BlockNumber,
}

impl Default for OperationQueue {
    fn default() -> Self {
        Self {
            last_block_number: BlockNumber(0),
            elements: VecDeque::new(),
        }
    }
}

impl OperationQueue {
    /// Creates a new empty counter queue with the custom `last_block_number`.
    pub fn new(last_block_number: BlockNumber) -> Self {
        Self {
            last_block_number,
            ..Default::default()
        }
    }

    /// Returns a previously popped element to the front of the queue.
    pub fn return_popped(&mut self, element: TxData) -> anyhow::Result<()> {
        if *self.last_block_number != *element.get_block_range().1 {
            return Err(format_err!("Insert an element that affects the block numbered NOT equal to the last in the queue predecessor"));
        }

        self.last_block_number = BlockNumber(*element.get_block_range().0 - 1);
        self.elements.push_front(element);

        Ok(())
    }

    /// Inserts an element to the end of the queue.
    pub fn push_back(&mut self, element: TxData) -> anyhow::Result<()> {
        let next_block_number = BlockNumber(
            *self
                .elements
                .back()
                .map(|element| element.get_block_range().1)
                .unwrap_or(self.last_block_number)
                + 1,
        );

        if *next_block_number != *element.get_block_range().0 {
            return Err(format_err!(
                "Insert an element that affects on not subsequent blocks"
            ));
        }
        self.elements.push_back(element);

        Ok(())
    }

    /// Attempts to take the next element from the queue. Returns `None`
    /// if the queue is empty.
    ///
    /// Taking the actual value updates the last affected block.
    pub fn pop_front(&mut self) -> Option<TxData> {
        match self.elements.pop_front() {
            Some(element) => {
                self.last_block_number = element.get_block_range().1;
                Some(element)
            }
            None => None,
        }
    }

    /// Returns the value of the last affected block.
    pub fn get_last_block_number(&self) -> BlockNumber {
        self.last_block_number
    }

    /// Returns the value of the next affected block
    /// if will pop the top item out of the queue.
    pub fn get_next_last_block_number(&self) -> Option<BlockNumber> {
        self.elements
            .front()
            .map(|element| element.get_block_range().1)
    }

    /// Returns the current size of the queue.
    pub fn len(&self) -> usize {
        self.elements.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zksync_storage::test_data::{gen_unique_aggregated_operation, BLOCK_SIZE_CHUNKS};
    use zksync_types::aggregated_operations::AggregatedActionType;

    /// Checks the main operations of the queue: `push_back`, `pop_front` and `get_count`.
    #[test]
    fn basic_operations() {
        let mut queue: OperationQueue = OperationQueue::new(BlockNumber(0));

        // Create aggregate operations.

        let tx_data_1 = {
            let op_1 = gen_unique_aggregated_operation(
                BlockNumber(1),
                AggregatedActionType::CommitBlocks,
                BLOCK_SIZE_CHUNKS,
            );
            TxData::from_operation((1, op_1), Default::default())
        };
        let tx_data_2 = {
            let op_2 = gen_unique_aggregated_operation(
                BlockNumber(2),
                AggregatedActionType::CommitBlocks,
                BLOCK_SIZE_CHUNKS,
            );
            TxData::from_operation((2, op_2), Default::default())
        };

        assert_eq!(queue.get_last_block_number(), BlockNumber(0));
        assert!(queue.get_next_last_block_number().is_none());

        // Insert the next element and obtain it.
        queue.push_back(tx_data_1.clone()).unwrap();
        // Inserting the element should NOT update the last block number.
        assert_eq!(queue.get_last_block_number(), BlockNumber(0));
        assert_eq!(queue.get_next_last_block_number().unwrap(), BlockNumber(1));
        assert_eq!(queue.pop_front().unwrap(), tx_data_1);
        // After taking the element, the counter should be updated.
        assert_eq!(queue.get_last_block_number(), BlockNumber(1));
        assert_eq!(queue.get_last_block_number(), BlockNumber(1));
        assert!(queue.get_next_last_block_number().is_none());

        // Perform the same check again and check that overall counter will become 2.
        queue.push_back(tx_data_2.clone()).unwrap();
        assert_eq!(queue.get_last_block_number(), BlockNumber(1));
        assert_eq!(queue.get_next_last_block_number().unwrap(), BlockNumber(2));

        assert_eq!(queue.pop_front().unwrap(), tx_data_2);
        assert_eq!(queue.get_last_block_number(), BlockNumber(2));
        assert!(queue.get_next_last_block_number().is_none());

        // Now attempt take no value, and check that counter is not increased.
        assert_eq!(queue.pop_front(), None);
        assert_eq!(queue.get_last_block_number(), BlockNumber(2));

        // Return the popped element back.
        queue.return_popped(tx_data_2.clone()).unwrap();
        assert_eq!(queue.get_last_block_number(), BlockNumber(1));
        assert_eq!(queue.get_last_block_number(), BlockNumber(1));
        assert_eq!(queue.get_next_last_block_number().unwrap(), BlockNumber(2));

        assert_eq!(queue.pop_front().unwrap(), tx_data_2);
        assert_eq!(queue.get_last_block_number(), BlockNumber(2));
        assert!(queue.get_next_last_block_number().is_none());
    }
}
