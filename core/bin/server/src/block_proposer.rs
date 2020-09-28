//! Block proposer is main driver of the application, it polls transactions from mempool
//! and sends them to `StateKeeper`
//!
//! It does it in small batches, called here `miniblocks`, which are smaller that full blocks.
//!
//! Right now logic of this actor is simple, but in future consensus will replace it using the same API.

// External deps
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use tokio::{task::JoinHandle, time};
// Workspace deps
use zksync_config::ConfigurationOptions;
// Local deps
use crate::{
    mempool::{GetBlockRequest, MempoolRequest, ProposedBlock},
    state_keeper::StateKeeperRequest,
};

fn create_mempool_req(
    last_priority_op_number: u64,
) -> (MempoolRequest, oneshot::Receiver<ProposedBlock>) {
    let (response_sender, receiver) = oneshot::channel();
    (
        MempoolRequest::GetBlock(GetBlockRequest {
            last_priority_op_number,
            response_sender,
        }),
        receiver,
    )
}

struct BlockProposer {
    current_priority_op_number: u64,

    mempool_requests: mpsc::Sender<MempoolRequest>,
    statekeeper_requests: mpsc::Sender<StateKeeperRequest>,
}

impl BlockProposer {
    async fn propose_new_block(&mut self) -> ProposedBlock {
        let (mempool_req, resp) = create_mempool_req(self.current_priority_op_number);
        self.mempool_requests
            .send(mempool_req)
            .await
            .expect("mempool receiver dropped");

        resp.await.expect("Mempool new block request failed")
    }

    async fn commit_new_tx_mini_batch(&mut self) {
        let proposed_block = self.propose_new_block().await;

        self.current_priority_op_number += proposed_block.priority_ops.len() as u64;
        self.statekeeper_requests
            .send(StateKeeperRequest::ExecuteMiniBlock(proposed_block))
            .await
            .expect("state keeper receiver dropped");
    }
}

// driving engine of the application
#[must_use]
pub fn run_block_proposer_task(
    config_options: &ConfigurationOptions,
    mempool_requests: mpsc::Sender<MempoolRequest>,
    mut statekeeper_requests: mpsc::Sender<StateKeeperRequest>,
) -> JoinHandle<()> {
    let miniblock_interval = config_options
        .miniblock_timings
        .miniblock_iteration_interval;
    tokio::spawn(async move {
        let mut timer = time::interval(miniblock_interval);

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

        let mut block_proposer = BlockProposer {
            current_priority_op_number,
            mempool_requests,
            statekeeper_requests,
        };

        loop {
            timer.tick().await;

            block_proposer.commit_new_tx_mini_batch().await;
        }
    })
}
