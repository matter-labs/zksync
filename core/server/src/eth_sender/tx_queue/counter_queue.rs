// TODO: Remove me
#![allow(dead_code)]

use std::{collections::VecDeque, fmt};

/// Counter queue is basically a queue which
/// tracks the amount of the processed elements.
#[derive(Debug)]
pub struct CounterQueue<T: fmt::Debug> {
    elements: VecDeque<T>,
    counter: usize,
}

impl<T: fmt::Debug> Default for CounterQueue<T> {
    fn default() -> Self {
        Self {
            counter: 0,
            elements: VecDeque::new(),
        }
    }
}

impl<T: fmt::Debug> CounterQueue<T> {
    /// Creates a new empty counter queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new empty counter queue with the custom number of processed elements.
    /// This method is used to restore the state of the queue.
    pub fn new_with_count(counter: usize) -> Self {
        Self {
            counter,
            ..Default::default()
        }
    }

    /// Inserts an element to the end of the queue.
    pub fn push_back(&mut self, element: T) {
        self.elements.push_back(element);
        self.counter += 1;
    }

    /// Attempts to take the next element from the queue. Returns `None`
    /// if the queue is empty.
    pub fn pop_front(&mut self) -> Option<T> {
        self.elements.pop_front()
    }

    /// Returns the value of the counter.
    pub fn get_count(&self) -> usize {
        self.counter
    }
}
