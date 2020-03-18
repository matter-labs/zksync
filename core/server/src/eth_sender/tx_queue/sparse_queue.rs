// TODO: Remove me
#![allow(dead_code)]

use std::{collections::HashMap, fmt};

/// Sparse queue is a sparse queue which allows inserting an element
/// with index (N + 1) when element with index N is not yet inserted.
///
/// Operation `pop_front` for this queue will not yield (N + 1) element
/// until the gap is filled, but once it's filled it will yield both
/// N and (N + 1) elements.
#[derive(Debug)]
pub struct SparseQueue<T: fmt::Debug> {
    current_idx: usize,
    elements: HashMap<usize, T>,
}

impl<T: fmt::Debug> Default for SparseQueue<T> {
    fn default() -> Self {
        Self {
            current_idx: 0,
            elements: HashMap::new(),
        }
    }
}

impl<T: fmt::Debug> SparseQueue<T> {
    /// Creates a new empty sparse queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new empty sparse queue with the custom expected next ID.
    /// This method is used to restore the state of the queue.
    pub fn new_from(idx: usize) -> Self {
        Self {
            current_idx: idx,
            ..Default::default()
        }
    }

    /// Inserts an element to the queue given its index.
    pub fn insert(&mut self, idx: usize, element: T) {
        assert!(
            idx >= self.current_idx,
            "Can't insert the element with index lower than the next expected one"
        );
        self.elements.insert(idx, element);
    }

    /// Attempts to take the next element from the queue. Returns `None`
    /// if either the queue is empty, or the next expected element is yet
    /// missing in the queue.
    pub fn pop_front(&mut self) -> Option<T> {
        match self.elements.remove(&self.current_idx) {
            Some(value) => {
                self.current_idx += 1;
                Some(value)
            }
            None => None,
        }
    }

    /// Checks whether `pop_front` operation will return an element or not.
    /// Returns `true` if the next expected element exists in the queue,
    /// and returns `false` otherwise.
    pub fn has_next(&self) -> bool {
        self.elements.contains_key(&self.current_idx)
    }

    /// Returns the next expected element ID.
    pub fn next_id(&self) -> usize {
        self.current_idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks the main operations of the queue: `insert`, `pop_front` and `has_next`.
    #[test]
    fn basic_operations() {
        let mut queue: SparseQueue<String> = SparseQueue::new();

        // Insert the next element and obtain it.
        queue.insert(0, "zero".into());
        assert!(queue.has_next());
        assert_eq!(queue.pop_front().unwrap(), "zero");

        // Now insert an element with a gap, and check that it won't be yielded.
        queue.insert(2, "two".into());
        assert!(!queue.has_next());
        assert!(queue.pop_front().is_none());

        // Now fill the gap and obtain both elements.
        queue.insert(1, "one".into());
        assert!(queue.has_next());
        assert_eq!(queue.pop_front().unwrap(), "one");
        assert!(queue.has_next());
        assert_eq!(queue.pop_front().unwrap(), "two");
    }
}
