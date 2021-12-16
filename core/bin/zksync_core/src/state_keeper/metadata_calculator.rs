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
use zksync_types::{
    block::{Block, BlockMetadata},
    BlockNumber, ExecutedOperations, H256,
};

use crate::committer::{BlockCommitRequest, CommitRequest};

use super::{pending_block::PendingBlock, types::StateKeeperConfig};

#[derive(Debug, Default, Clone)]
pub(super) struct JobQueue {
    queue: Arc<Mutex<VecDeque<PendingBlock>>>,
    size: Arc<AtomicUsize>,
}

impl JobQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub(super) async fn push(&mut self, job: PendingBlock) {
        self.queue.lock().await.push_back(job);
        // Here and below: `Relaxed` is enough as don't rely on the value for any critical sections.
        self.size.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) async fn pop(&mut self) -> Option<PendingBlock> {
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

    async fn process_job(&mut self, mut pending_block: PendingBlock) {
        // Ensure that the new block has expected number.
        assert_eq!(
            pending_block.number,
            self.last_block_number + 1,
            "Got unexpected block to process. Expected block #{}, got #{}. \n \
             Pending block contents: {:?}",
            self.last_block_number + 1,
            pending_block.number,
            pending_block
        );

        // Update the state stored in self.
        self.state
            .apply_account_updates(pending_block.account_updates.clone());

        // Form the block and send it.
        let mut block_transactions = pending_block.success_operations.clone(); // TODO (in this PR): Avoid cloning.
        block_transactions.extend(
            pending_block
                .failed_txs
                .iter()
                .cloned() // TODO (in this PR): Avoid cloning.
                .map(|tx| ExecutedOperations::Tx(Box::new(tx))),
        );

        let commit_gas_limit = pending_block.gas_counter.commit_gas_limit();
        let verify_gas_limit = pending_block.gas_counter.verify_gas_limit();

        let block = Block::new_from_available_block_sizes(
            pending_block.number,
            self.state.root_hash(),
            self.config.fee_account_id,
            block_transactions,
            (
                pending_block.unprocessed_priority_op_before,
                pending_block.unprocessed_priority_op_current,
            ),
            &self.config.available_block_chunk_sizes,
            commit_gas_limit,
            verify_gas_limit,
            self.last_root_hash,
            pending_block.timestamp,
        );

        // Update the fields of the new pending block.
        *self.last_block_number += 1;
        self.last_root_hash = block.get_eth_encoded_root();

        let block_metadata = BlockMetadata {
            fast_processing: pending_block.fast_processing_required,
        };

        let block_commit_request = BlockCommitRequest {
            block,
            block_metadata,
            accounts_updated: pending_block.account_updates.clone(),
        };
        let applied_updates_request = pending_block.prepare_applied_updates_request();

        vlog::info!(
            "Creating full block: {}, operations: {}, chunks_left: {}, miniblock iterations: {}",
            *block_commit_request.block.block_number,
            block_commit_request.block.block_transactions.len(),
            pending_block.chunks_left,
            pending_block.pending_block_iteration
        );

        let commit_request = CommitRequest::Block((block_commit_request, applied_updates_request));
        self.tx_for_commitments
            .send(commit_request)
            .await
            .expect("committer receiver dropped");
    }
}
