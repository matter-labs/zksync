// Built-in imports
use crate::eth_sender::tx_queue::TxData;
use std::collections::HashMap;

#[derive(Debug)]
pub struct VerifyQueue {
    next_expected_idx: usize,
    elements: HashMap<usize, TxData>,
}

impl Default for VerifyQueue {
    fn default() -> Self {
        Self {
            next_expected_idx: 0,
            elements: HashMap::new(),
        }
    }
}

impl VerifyQueue {
    /// Creates a new empty verify queue with the custom next expected element ID.
    pub fn new(next_expected_idx: usize) -> Self {
        Self {
            next_expected_idx,
            ..Default::default()
        }
    }

    /// Returns a previously popped element to the front of the queue.
    pub fn return_popped(&mut self, element: TxData) {
        let popped_index = self.next_expected_idx
            - element
                .op_type
                .number_of_block_to_verify()
                .expect("must be a verify operation");
        self.elements.insert(popped_index, element);
        self.next_expected_idx = popped_index;
    }

    /// Inserts an element to the queue given its index.
    pub fn insert(&mut self, idx: usize, element: TxData) {
        assert!(
            idx >= self.next_expected_idx,
            "Can't insert the element with index lower than the next expected one"
        );
        self.elements.insert(idx, element);
    }

    /// Attempts to take the next element from the queue. Returns `None`
    /// if either the queue is empty, or the next expected element is yet
    /// missing in the queue.
    pub fn pop_front(&mut self) -> Option<TxData> {
        match self.elements.remove(&self.next_expected_idx) {
            Some(value) => {
                self.next_expected_idx += value
                    .op_type
                    .number_of_block_to_verify()
                    .expect("must be a verify operation");
                Some(value)
            }
            None => None,
        }
    }

    pub fn front_element(&self) -> Option<TxData> {
        return self.elements.get(&self.next_expected_idx).cloned();
    }

    /// Returns the next expected element ID.
    pub fn next_id(&self) -> usize {
        self.next_expected_idx
    }

    /// Returns the current size of the queue.
    pub fn len(&self) -> usize {
        self.elements.len()
    }
}
