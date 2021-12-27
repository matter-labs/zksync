use std::time::{Duration, Instant};

use futures::{channel::mpsc, SinkExt};
use tokio::task::JoinHandle;

use zksync_state::state::ZkSyncState;
use zksync_types::BlockNumber;

use crate::committer::{BlockFinishRequest, CommitRequest};

mod queue;

pub use self::queue::{BlockRootHashJob, BlockRootHashJobQueue};

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
pub struct RootHashCalculator {
    state: ZkSyncState,
    // We use job queue to be able to observe amount of not-yet-calculated jobs
    // so we can throttle performance if needed.
    job_queue: BlockRootHashJobQueue,
    tx_for_commitments: mpsc::Sender<CommitRequest>,

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
        // Calculate the root hash, so the tree cache for the current state is calculated.
        let _last_root_hash = state.root_hash();

        Self {
            state,
            job_queue,
            tx_for_commitments,
            last_block_number,
        }
    }

    pub async fn run(mut self) {
        loop {
            // TODO (ZKS-857): using `sleep` is inefficient. We need `job_queue.pop()` to resolve *only* when we have
            // a new job available.
            tokio::time::sleep(Duration::from_millis(25)).await;
            if let Some(job) = self.job_queue.pop().await {
                self.process_job(job).await;
            }
        }
    }

    async fn process_job(&mut self, job: BlockRootHashJob) {
        vlog::info!("Received job to process block #{}", job.block);
        let start = Instant::now();

        // Ensure that the new block has expected number.
        assert_eq!(
            job.block,
            self.last_block_number + 1,
            "Got unexpected block to process."
        );

        // Update the state stored in self.
        self.state.apply_account_updates(job.updates);

        let root_hash = self.state.root_hash();

        vlog::info!("Root hash for block #{} is calculated", job.block);

        let finalize_request = CommitRequest::FinishBlock(BlockFinishRequest {
            block_number: job.block,
            root_hash,
        });
        self.tx_for_commitments
            .send(finalize_request)
            .await
            .expect("committer receiver dropped");

        // Increment block number to expect the next one.
        self.last_block_number = self.last_block_number + 1;

        metrics::histogram!("root_hash_calculator.process_job", start.elapsed());
        metrics::gauge!(
            "root_hash_calculator.last_processed_block",
            job.block.0 as f64
        );
    }
}

#[must_use]
pub fn start_root_hash_calculator(rhc: RootHashCalculator) -> JoinHandle<()> {
    tokio::spawn(rhc.run())
}
