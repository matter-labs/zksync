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

use crate::committer::CommitRequest;

use super::types::StateKeeperConfig;

#[derive(Debug)]
pub(super) struct Job {
    pub(super) block: BlockNumber,
    pub(super) updates: AccountUpdates,
}

#[derive(Debug, Default, Clone)]
pub(super) struct JobQueue {
    queue: Arc<Mutex<VecDeque<Job>>>,
    size: Arc<AtomicUsize>,
}

impl JobQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub(super) async fn push(&mut self, job: Job) {
        self.queue.lock().await.push_back(job);
        // Here and below: `Relaxed` is enough as don't rely on the value for any critical sections.
        self.size.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) async fn pop(&mut self) -> Option<Job> {
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

    pub(super) fn size(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }
}

#[derive(Debug)]
pub(super) struct MetadataCalculator {
    state: ZkSyncState,
    // We use job queue to be able to observe amount of not-yet-calculated jobs
    // so we can throttle performance if needed.
    job_queue: JobQueue,
    tx_for_commitments: mpsc::Sender<CommitRequest>,

    // We process block sequentially, so we can store the previous root hash needed
    // to form the block header.
    last_root_hash: H256,
    // While we don't really need the number for calculations, it's useful for safety
    // to ensure that every block is processed in order.
    last_block_number: BlockNumber,

    config: StateKeeperConfig,
}

impl MetadataCalculator {
    pub(super) fn new(
        state: ZkSyncState,
        job_queue: JobQueue,
        tx_for_commitments: mpsc::Sender<CommitRequest>,
        last_block_number: BlockNumber,
        config: StateKeeperConfig,
    ) -> Self {
        let last_root_hash = Block::encode_fr_for_eth(state.root_hash());

        Self {
            state,
            job_queue,
            tx_for_commitments,
            last_root_hash,
            last_block_number,
            config,
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

    async fn process_job(&mut self, job: Job) {
        // Ensure that the new block has expected number.
        assert_eq!(
            job.block,
            self.last_block_number + 1,
            "Got unexpected block to process."
        );

        // Update the state stored in self.
        self.state.apply_account_updates(job.updates);

        let root_hash = self.state.root_hash();

        let finalize_request = CommitRequest::FinishBlock((job.block, root_hash));
        self.tx_for_commitments
            .send(finalize_request)
            .await
            .expect("committer receiver dropped");
    }
}
