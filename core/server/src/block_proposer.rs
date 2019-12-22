use crate::mempool::{GetBlockRequest, MempoolRequest, ProposedBlock};
use crate::state_keeper::StateKeeperRequest;
use futures::channel::{mpsc, oneshot};
use futures::SinkExt;
use models::params::block_size_chunks;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::time;

const TX_MINIBATCH_CREATE_TIME: Duration = Duration::from_millis(5000);

fn create_mempool_req(
    last_priority_op_number: u64,
    chunks: usize,
) -> (MempoolRequest, oneshot::Receiver<ProposedBlock>) {
    let (response_sender, receiver) = oneshot::channel();

    (
        MempoolRequest::GetBlock(GetBlockRequest {
            last_priority_op_number,
            chunks,
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
        // TODO: normal number
        let (mempool_req, resp) =
            create_mempool_req(self.current_priority_op_number, block_size_chunks());
        self.mempool_requests
            .send(mempool_req)
            .await
            .expect("mempool receiver dropped");

        // TODO: unwrap
        resp.await.unwrap()
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
pub fn run_block_proposer_task(
    mempool_requests: mpsc::Sender<MempoolRequest>,
    mut statekeeper_requests: mpsc::Sender<StateKeeperRequest>,
    runtime: &Runtime,
) {
    // TODO: proper const

    runtime.spawn(async move {
        let mut timer = time::interval(TX_MINIBATCH_CREATE_TIME);

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
    });
}
