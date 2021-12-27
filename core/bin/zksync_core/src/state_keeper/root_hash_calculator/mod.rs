use futures::{channel::mpsc, SinkExt};
use tokio::task::JoinHandle;

use zksync_state::state::ZkSyncState;
use zksync_types::{block::Block, BlockNumber, H256};

use crate::committer::{BlockFinishRequest, CommitRequest};

mod queue;

pub use self::queue::BlockRootHashJob;
pub(in crate::state_keeper) use self::queue::BlockRootHashJobQueue;

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

    async fn process_job(&mut self, job: BlockRootHashJob) {
        vlog::info!("Received job to process block #{}", job.block);

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
    }
}

#[must_use]
pub fn start_root_hash_calculator(rhc: RootHashCalculator) -> JoinHandle<()> {
    tokio::spawn(rhc.run())
}
