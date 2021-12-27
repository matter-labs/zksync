//! Block proposer is main driver of the application, it polls transactions from mempool
//! and sends them to `StateKeeper`
//!
//! It does it in small batches, called here `miniblocks`, which are smaller that full blocks.
//!
//! Right now logic of this actor is simple, but in future consensus will replace it using the same API.

use std::time::{Duration, Instant};

// External deps
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use tokio::{task::JoinHandle, time};
// Workspace deps
use zksync_config::ZkSyncConfig;
// Local deps
use crate::{
    mempool::{GetBlockRequest, MempoolBlocksRequest, ProposedBlock},
    state_keeper::{BlockRootHashJobQueue, StateKeeperRequest},
};

fn create_mempool_req(
    last_priority_op_number: u64,
    block_timestamp: u64,
) -> (MempoolBlocksRequest, oneshot::Receiver<ProposedBlock>) {
    let (response_sender, receiver) = oneshot::channel();
    (
        MempoolBlocksRequest::GetBlock(GetBlockRequest {
            last_priority_op_number,
            block_timestamp,
            response_sender,
        }),
        receiver,
    )
}

struct BlockProposer {
    root_hash_queue: BlockRootHashJobQueue,
    current_priority_op_number: u64,

    mempool_requests: mpsc::Sender<MempoolBlocksRequest>,
    statekeeper_requests: mpsc::Sender<StateKeeperRequest>,
}

impl BlockProposer {
    async fn run(mut self, miniblock_interval: Duration) {
        let mut timer = time::interval(miniblock_interval);
        loop {
            timer.tick().await;

            let start = Instant::now();

            // `.throttle()` method will postpone the next miniblock iteration if currently we have too
            // many blocks for which root hash is not yet calculated.
            self.root_hash_queue.throttle().await;
            metrics::histogram!("block_proposer.throttle", start.elapsed());

            self.commit_new_tx_mini_batch().await;
        }
    }

    async fn propose_new_block(&mut self, block_timestamp: u64) -> ProposedBlock {
        let (mempool_req, resp) =
            create_mempool_req(self.current_priority_op_number, block_timestamp);
        self.mempool_requests
            .send(mempool_req)
            .await
            .expect("mempool receiver dropped");

        resp.await.expect("Mempool new block request failed")
    }

    async fn get_pending_block_timestamp(&mut self) -> u64 {
        let (block_timestamp_sender, block_timestamp_receiver) = oneshot::channel();
        self.statekeeper_requests
            .send(StateKeeperRequest::GetPendingBlockTimestamp(
                block_timestamp_sender,
            ))
            .await
            .expect("state keeper receiver dropped");

        block_timestamp_receiver
            .await
            .expect("State keeper pending block timestamp request failed")
    }

    async fn commit_new_tx_mini_batch(&mut self) {
        let block_timestamp = self.get_pending_block_timestamp().await;
        let proposed_block = self.propose_new_block(block_timestamp).await;

        self.current_priority_op_number += proposed_block.priority_ops.len() as u64;
        self.statekeeper_requests
            .send(StateKeeperRequest::ExecuteMiniBlock(proposed_block))
            .await
            .expect("state keeper receiver dropped");
    }
}

// driving engine of the application
#[must_use]
pub(crate) fn run_block_proposer_task(
    config: &ZkSyncConfig,
    mempool_requests: mpsc::Sender<MempoolBlocksRequest>,
    mut statekeeper_requests: mpsc::Sender<StateKeeperRequest>,
    root_hash_queue: BlockRootHashJobQueue,
) -> JoinHandle<()> {
    let miniblock_interval = config.chain.state_keeper.miniblock_iteration_interval();
    tokio::spawn(async move {
        let last_unprocessed_prior_op_chan = oneshot::channel();
        statekeeper_requests
            .send(StateKeeperRequest::GetLastUnprocessedPriorityOp(
                last_unprocessed_prior_op_chan.0,
            ))
            .await
            .expect("state keeper receiver dropped");
        let current_priority_op_number = last_unprocessed_prior_op_chan
            .1
            .await
            .expect("Unprocessed priority op initialization");

        let block_proposer = BlockProposer {
            current_priority_op_number,
            root_hash_queue,
            mempool_requests,
            statekeeper_requests,
        };

        block_proposer.run(miniblock_interval).await;
    })
}
