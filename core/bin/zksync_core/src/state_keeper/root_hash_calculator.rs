use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use futures::{channel::mpsc, SinkExt};
use tokio::sync::Mutex;

use zksync_state::state::ZkSyncState;
use zksync_types::{block::Block, AccountUpdates, BlockNumber, H256};

use crate::committer::{BlockFinishRequest, CommitRequest};

/// Description of a single block root hash job.
///
/// Contains data required to calculate the root hash of a block, given that root hashes
/// are calculated sequentially and calculator maintains the copy of the blockchain state.
#[derive(Debug, Clone)]
pub struct BlockRootHashJob {
    /// Number of the block. While not required to calculate the root hash,
    /// it is used to ensure that no block was missed.
    pub(super) block: BlockNumber,
    /// Account updates that happened in the block.
    pub(super) updates: AccountUpdates,
}

/// Queue of jobs for calculating block root hashes.
///
/// Unlike channel, it provides a way to see the queue size, which can be used
/// to throttle transaction execution if blocks are being created faster than it is
/// possible to process them.
#[derive(Debug, Default, Clone)]
pub(super) struct BlockRootHashJobQueue {
    queue: Arc<Mutex<VecDeque<BlockRootHashJob>>>,
    size: Arc<AtomicUsize>,
}

impl BlockRootHashJobQueue {
    /// Creates a new empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an element to the queue.
    pub(super) async fn push(&mut self, job: BlockRootHashJob) {
        self.queue.lock().await.push_back(job);
        // Here and below: `Relaxed` is enough as don't rely on the value for any critical sections.
        self.size.fetch_add(1, Ordering::Relaxed);
    }

    /// Pops an element from the queue.
    pub(super) async fn pop(&mut self) -> Option<BlockRootHashJob> {
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
    pub(super) fn size(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }
}

/// Entity capable of calculating the root hashes and sending information
/// to the committer in order to complete the incomplete blocks.
///
/// It is supposed to run in parallel with the state keeper, so the state keeper
/// can keep creating blocks without having to wait for the root hash to be calculated.
///
/// Approach of `RootHashCalculator` will work as long as we time to create a new block
/// is bigger than time required to calculate the root hash of said block. Otherwise, we
/// will create blocks on a faster pace than we can process them.
///
/// It is important that if we use this entity, we should have a throttling mechanism which
/// will stop transaction execution if blocks are being created too fast.
#[derive(Debug)]
pub(super) struct RootHashCalculator {
    state: ZkSyncState,
    // We use job queue to be able to observe amount of not-yet-calculated jobs
    // so we can throttle performance if needed.
    job_queue: BlockRootHashJobQueue,
    tx_for_commitments: mpsc::Sender<CommitRequest>,

    // We process block sequentially, so we can store the previous root hash needed
    // to form the block header.
    last_root_hash: H256,
    // While we don't really need the number for calculations, it's useful for safety
    // to ensure that every block is processed in order.
    last_block_number: BlockNumber,
}

impl RootHashCalculator {
    pub(super) fn new(
        state: ZkSyncState,
        job_queue: BlockRootHashJobQueue,
        tx_for_commitments: mpsc::Sender<CommitRequest>,
        last_block_number: BlockNumber,
    ) -> Self {
        let last_root_hash = Block::encode_fr_for_eth(state.root_hash());

        Self {
            state,
            job_queue,
            tx_for_commitments,
            last_root_hash,
            last_block_number,
        }
    }

    pub(super) async fn run(mut self) {
        loop {
            // TODO: using `sleep` is inefficient. We need `job_queue.pop()` to resolve *only* when we have
            // a new job available.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if let Some(job) = self.job_queue.pop().await {
                self.process_job(job).await;
            }
        }
    }

    async fn process_job(&mut self, job: BlockRootHashJob) {
        // Ensure that the new block has expected number.
        assert_eq!(
            job.block,
            self.last_block_number + 1,
            "Got unexpected block to process."
        );

        // Update the state stored in self.
        self.state.apply_account_updates(job.updates);

        let root_hash = self.state.root_hash();

        let finalize_request = CommitRequest::FinishBlock(BlockFinishRequest {
            block_number: job.block,
            root_hash,
        });
        self.tx_for_commitments
            .send(finalize_request)
            .await
            .expect("committer receiver dropped");
    }
}
