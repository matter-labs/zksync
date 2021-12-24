use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use tokio::sync::Mutex;

use zksync_types::{AccountUpdates, BlockNumber};

/// Description of a single block root hash job.
///
/// Contains data required to calculate the root hash of a block, given that root hashes
/// are calculated sequentially and calculator maintains the copy of the blockchain state.
#[derive(Debug, Clone)]
pub struct BlockRootHashJob {
    /// Number of the block. While not required to calculate the root hash,
    /// it is used to ensure that no block was missed.
    pub(crate) block: BlockNumber,
    /// Account updates that happened in the block.
    pub(crate) updates: AccountUpdates,
}

/// Queue of jobs for calculating block root hashes.
///
/// Unlike channel, it provides a way to see the queue size, which can be used
/// to throttle transaction execution if blocks are being created faster than it is
/// possible to process them.
#[derive(Debug, Default, Clone)]
pub(crate) struct BlockRootHashJobQueue {
    queue: Arc<Mutex<VecDeque<BlockRootHashJob>>>,
    size: Arc<AtomicUsize>,
}

impl BlockRootHashJobQueue {
    /// Creates a filled queue.
    pub(crate) fn new(jobs: impl IntoIterator<Item = BlockRootHashJob>) -> Self {
        let queue: VecDeque<_> = jobs.into_iter().collect();
        let size = queue.len();
        Self {
            queue: Arc::new(Mutex::new(queue)),
            size: Arc::new(AtomicUsize::from(size)),
        }
    }

    /// Adds an element to the queue.
    pub(crate) async fn push(&mut self, job: BlockRootHashJob) {
        self.queue.lock().await.push_back(job);
        // Here and below: `Relaxed` is enough as don't rely on the value for any critical sections.
        self.size.fetch_add(1, Ordering::Relaxed);
    }

    /// Pops an element from the queue.
    pub(crate) async fn pop(&mut self) -> Option<BlockRootHashJob> {
        let result = self.queue.lock().await.pop_front();
        if result.is_some() {
            let old_value = self.size.fetch_sub(1, Ordering::Relaxed);
            // This variant is logically impossible (we can't pop more elements than we added),
            // but it's still preferable to check for underflows.
            assert!(
                old_value != 0,
                "Underflow on job queue size in state keeper"
            );
        }
        result
    }

    /// Returns the current size of the queue.
    pub(crate) fn size(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks basic block jobs queue functionality.
    #[tokio::test]
    async fn queue_functionality() {
        let mut queue = BlockRootHashJobQueue::new(std::iter::empty());
        assert_eq!(queue.size(), 0);

        queue
            .push(BlockRootHashJob {
                block: BlockNumber(1),
                updates: Vec::new(),
            })
            .await;
        assert_eq!(queue.size(), 1);

        queue
            .push(BlockRootHashJob {
                block: BlockNumber(2),
                updates: Vec::new(),
            })
            .await;
        assert_eq!(queue.size(), 2);

        let first_job = queue.pop().await.expect("Should pop element");
        assert_eq!(first_job.block, BlockNumber(1));
        assert_eq!(queue.size(), 1);

        let second_job = queue.pop().await.expect("Should pop element");
        assert_eq!(second_job.block, BlockNumber(2));
        assert_eq!(queue.size(), 0);

        assert!(queue.pop().await.is_none(), "No elements left");
        assert_eq!(
            queue.size(),
            0,
            "Size should not change after popping from empty"
        );
    }
}
