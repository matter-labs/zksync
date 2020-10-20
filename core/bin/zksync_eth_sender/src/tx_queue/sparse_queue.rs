// Built-in imports
use std::{collections::HashMap, fmt};

/// Sparse queue is a sparse queue which allows inserting an element
/// with index (N + 1) when element with index N is not yet inserted.
///
/// Operation `pop_front` for this queue will not yield (N + 1) element
/// until the gap is filled, but once it's filled it will yield both
/// N and (N + 1) elements.
#[derive(Debug)]
pub struct SparseQueue<T: fmt::Debug> {
    next_expected_idx: usize,
    pub(super) elements: HashMap<usize, T>,
}

impl<T: fmt::Debug> Default for SparseQueue<T> {
    fn default() -> Self {
        Self {
            next_expected_idx: 0,
            elements: HashMap::new(),
        }
    }
}

impl<T: fmt::Debug> SparseQueue<T> {
    /// Creates a new empty sparse queue with the custom next expected element ID.
    pub fn new(next_expected_idx: usize) -> Self {
        Self {
            next_expected_idx,
            ..Default::default()
        }
    }

    /// Returns a previously popped element to the front of the queue.
    pub fn return_popped(&mut self, element: T) {
        let popped_index = self.next_expected_idx - 1;
        self.elements.insert(popped_index, element);
        self.next_expected_idx = popped_index;
    }

    /// Inserts an element to the queue given its index.
    pub fn insert(&mut self, idx: usize, element: T) {
        assert!(
            idx >= self.next_expected_idx,
            "Can't insert the element with index lower than the next expected one"
        );
        self.elements.insert(idx, element);
    }

    /// Attempts to take the next element from the queue. Returns `None`
    /// if either the queue is empty, or the next expected element is yet
    /// missing in the queue.
    pub fn pop_front(&mut self) -> Option<T> {
        match self.elements.remove(&self.next_expected_idx) {
            Some(value) => {
                self.next_expected_idx += 1;
                Some(value)
            }
            None => None,
        }
    }

    /// Checks whether `pop_front` operation will return an element or not.
    /// Returns `true` if the next expected element exists in the queue,
    /// and returns `false` otherwise.
    pub fn has_next(&self) -> bool {
        self.elements.contains_key(&self.next_expected_idx)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks the main operations of the queue: `insert`, `pop_front` and `has_next`.
    #[test]
    fn basic_operations() {
        let mut queue: SparseQueue<String> = SparseQueue::new(0);

        // Insert the next element and obtain it.
        queue.insert(0, "zero".into());
        assert!(queue.has_next());
        assert_eq!(queue.next_id(), 0);
        assert_eq!(queue.pop_front().unwrap(), "zero");
        assert_eq!(queue.next_id(), 1);

        // Now insert an element with a gap, and check that it won't be yielded.
        queue.insert(2, "two".into());
        assert!(!queue.has_next());
        assert_eq!(queue.next_id(), 1);
        assert!(queue.pop_front().is_none());

        // Now fill the gap and obtain both elements.
        queue.insert(1, "one".into());
        assert!(queue.has_next());
        assert_eq!(queue.pop_front().unwrap(), "one");
        assert_eq!(queue.next_id(), 2);
        assert!(queue.has_next());
        assert_eq!(queue.pop_front().unwrap(), "two");
        assert_eq!(queue.next_id(), 3);

        // Return the popped element back.
        queue.return_popped("two".into());
        assert_eq!(queue.next_id(), 2);
        assert_eq!(queue.pop_front().unwrap(), "two");
        assert_eq!(queue.next_id(), 3);
    }

    /// Checks that we can use the difference `next_expected_idx` as the custom
    /// queue start point.
    #[test]
    fn different_start_point() {
        let mut queue: SparseQueue<String> = SparseQueue::new(10);

        // Check that by default the queue is empty.
        assert!(!queue.has_next());

        // Insert the next element and obtain it.
        queue.insert(10, "ten".into());
        assert!(queue.has_next());
        assert_eq!(queue.pop_front().unwrap(), "ten");
    }

    /// Checks that attempt to add the element with the index lower than
    /// the current expected ID results in panic.
    #[test]
    #[should_panic]
    fn add_out_of_order_element() {
        let mut queue: SparseQueue<String> = SparseQueue::new(10);
        // Insert the element with too low index.
        queue.insert(0, "zero".into());
    }
}
