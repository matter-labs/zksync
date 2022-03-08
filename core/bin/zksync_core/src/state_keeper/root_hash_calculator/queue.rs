use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use tokio::sync::{Mutex, Notify};

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
pub struct BlockRootHashJobQueue {
    /// Thread-safe shared queue.
    queue: Arc<Mutex<VecDeque<BlockRootHashJob>>>,
    /// Queue size.
    size: Arc<AtomicUsize>,
    /// New jobs notification channel. Used to wake up waiters when the new job was pushed to the queue.
    notify: Arc<Notify>,
}

impl BlockRootHashJobQueue {
    /// Creates a filled queue.
    pub(crate) fn new(jobs: impl IntoIterator<Item = BlockRootHashJob>) -> Self {
        let queue: VecDeque<_> = jobs.into_iter().collect();
        let size = queue.len();
        Self {
            queue: Arc::new(Mutex::new(queue)),
            size: Arc::new(AtomicUsize::from(size)),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Adds an element to the queue.
    pub(crate) async fn push(&mut self, job: BlockRootHashJob) {
        self.queue.lock().await.push_back(job);
        // Here and below: `Relaxed` is enough as don't rely on the value for any critical sections.
        self.size.fetch_add(1, Ordering::Relaxed);
        self.notify.notify_one();
        metrics::increment_gauge!("block_root_hash_job_queue.size", 1.0);
    }

    /// Pops an element from the queue.
    pub(crate) async fn pop(&mut self) -> BlockRootHashJob {
        loop {
            if let Some(result) = self.queue.lock().await.pop_front() {
                let old_value = self.size.fetch_sub(1, Ordering::Relaxed);
                // This variant is logically impossible (we can't pop more elements than we added),
                // but it's still preferable to check for underflows.
                assert!(
                    old_value != 0,
                    "Underflow on job queue size in state keeper"
                );
                metrics::decrement_gauge!("block_root_hash_job_queue.size", 1.0);
                return result;
            }

            // No job yet, wait for one to become available.
            self.notify.notified().await;
        }
    }

    /// Returns the current size of the queue.
    pub(crate) fn size(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    /// Returns whether we should stop the miniblock producing until the size of queue is decreased.
    pub(crate) fn should_throttle(&self) -> bool {
        // This method is going to be called by the block proposer, which does not know about block creation, so it
        // can be called in the random moment (e.g. right after the block was sealed and processing started).
        //
        // Note that as long as time to create a block is bigger than time to calculate root hash of the block (currently,
        // about 3 seconds on the production server), we won't thorttle often. This is a measure for situations when we
        // are close to the maximum server throughput. Throttling is implemented in a ways that makes us seal blocks with
        // the same pace as we calculate root hashes. By having `2` here, we ensure that the state keeper don't create
        // more than 1 block without the calculated root hash.
        //
        // We could've used the bigger number here to deal with sudden activity bursts, but it would perform worse if
        // we are constantly throttled (e.g. root hash calculator will always lag behind the state keeper), so for now
        // we stick to 2 as more "predictable" option.
        self.size() >= 2
    }

    /// Blocks until the job queue is small enough to proceed with the block generation.
    pub(crate) async fn throttle(&self) {
        // Duration interval should be small enough compared to the root hash calculation time, so that we
        // don't "overthrottle".
        const THROTTLE_ITERATION_INTERVAL: Duration = Duration::from_millis(25);

        // Note: since block proposer is already timeout-based, it is more or less OK to sleep here too.
        // If it will become a bottleneck (which is unlikely), we can implement a `Future` that resolves
        // when we the queue has enough elements.
        while self.should_throttle() {
            tokio::time::sleep(THROTTLE_ITERATION_INTERVAL).await;
        }
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
        assert!(!queue.should_throttle());

        queue
            .push(BlockRootHashJob {
                block: BlockNumber(1),
                updates: Vec::new(),
            })
            .await;
        assert_eq!(queue.size(), 1);
        assert!(!queue.should_throttle());

        queue
            .push(BlockRootHashJob {
                block: BlockNumber(2),
                updates: Vec::new(),
            })
            .await;
        assert_eq!(queue.size(), 2);
        assert!(queue.should_throttle());

        let first_job = queue.pop().await;
        assert_eq!(first_job.block, BlockNumber(1));
        assert_eq!(queue.size(), 1);
        assert!(!queue.should_throttle());

        let second_job = queue.pop().await;
        assert_eq!(second_job.block, BlockNumber(2));
        assert_eq!(queue.size(), 0);
        assert!(!queue.should_throttle());
    }

    /// Checks notifier logic.
    #[tokio::test]
    async fn queue_notifications() {
        let mut queue = BlockRootHashJobQueue::new(std::iter::empty());
        let mut queue2 = queue.clone();
        assert_eq!(queue.size(), 0);

        let task = tokio::spawn(async move {
            let first = queue2.pop().await;
            let second = queue2.pop().await;
            assert_eq!(first.block, BlockNumber(1));
            assert_eq!(second.block, BlockNumber(2));
        });

        // To create some time gap for the spawned task.
        tokio::time::sleep(Duration::from_millis(50)).await;

        queue
            .push(BlockRootHashJob {
                block: BlockNumber(1),
                updates: Vec::new(),
            })
            .await;
        queue
            .push(BlockRootHashJob {
                block: BlockNumber(2),
                updates: Vec::new(),
            })
            .await;

        let result = tokio::time::timeout(Duration::from_millis(100), task)
            .await
            .expect("Task time out");
        assert!(result.is_ok(), "Spawned task failed");
    }
}
