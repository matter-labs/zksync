use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use futures::channel::mpsc;
use tokio::sync::Mutex;

use zksync_state::state::ZkSyncState;
use zksync_types::AccountUpdates;

use crate::committer::CommitRequest;

#[derive(Debug)]
pub struct BlockJob {
    pub account_updates: AccountUpdates,
}

#[derive(Debug, Default, Clone)]
pub struct JobQueue {
    queue: Arc<Mutex<VecDeque<BlockJob>>>,
    size: Arc<AtomicUsize>,
}

impl JobQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn push(&mut self, job: BlockJob) {
        self.queue.lock().await.push_back(job);
        // Here and below: `Relaxed` is enough as don't rely on the value for any critical sections.
        self.size.fetch_add(1, Ordering::Relaxed);
    }

    pub async fn pop(&mut self) -> Option<BlockJob> {
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

    pub fn size(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }
}

#[derive(Debug)]
pub struct MetadataCalculator {
    state: ZkSyncState,
    // We use job queue to be able to observe amount of not-yet-calculated jobs
    // so we can throttle performance if needed.
    job_queue: JobQueue,
    tx_for_commitments: mpsc::Sender<CommitRequest>,
}

impl MetadataCalculator {
    pub fn new(
        state: ZkSyncState,
        job_queue: JobQueue,
        tx_for_commitments: mpsc::Sender<CommitRequest>,
    ) -> Self {
        Self {
            state,
            job_queue,
            tx_for_commitments,
        }
    }

    pub async fn run(mut self) {
        loop {
            // TODO: using `sleep` is inefficient. We need `job_queue.pop()` to resolve *only* when we have
            // a new job available.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if let Some(job) = self.job_queue.pop().await {
                self.process_job(job).await;
            }
        }
    }

    async fn process_job(&mut self, job: BlockJob) {
        self.state.apply_account_updates(job.account_updates);
    }
}
