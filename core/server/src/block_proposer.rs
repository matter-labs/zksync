use crate::mempool::{GetBlockRequest, MempoolRequest, ProposedBlock};
use crate::state_keeper::StateKeeperRequest;
use futures::channel::{mpsc, oneshot};
use futures::{SinkExt, StreamExt};
use models::node::config;
use models::params::block_size_chunks;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use tokio::time;

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

    /// Promised latest UNIX timestamp of the next block
    block_tries: usize,
    mempool_requests: mpsc::Sender<MempoolRequest>,
    statekeeper_requests: mpsc::Sender<StateKeeperRequest>,
}

impl BlockProposer {
    async fn propose_new_block(&mut self) -> ProposedBlock {
        // TODO: normal number
        let (mempool_req, resp) =
            create_mempool_req(self.current_priority_op_number, block_size_chunks());
        self.mempool_requests.send(mempool_req).await;

        // TODO: unwrap
        resp.await.unwrap()
    }

    /// Algorithm for creating new block
    /// At fixed time intervals: `PADDING_SUB_INTERVAL`
    /// 1) select executable transactions from mempool.
    /// 2.1) if # of executable txs == 0 => do nothing
    /// 2.2) if # of executable txs creates block that is filled for more than 4/5 of its capacity => commit
    /// 2.3) if # of executable txs creates block that is NOT filled for more than 4/5 of its capacity => wait for next time interval
    /// but no more than `BLOCK_FORMATION_TRIES`
    ///
    /// If we have only 1 tx next block will be at `now + PADDING_SUB_INTERVAL*BLOCK_FORMATION_TRIES`
    /// If we have a lot of txs to execute next block will be at  `now + PADDING_SUB_INTERVAL`
    async fn commit_new_block_or_wait_for_txs(&mut self) {
        let proposed_block = self.propose_new_block().await;
        if proposed_block.priority_ops.is_empty() && proposed_block.txs.is_empty() {
            return;
        }
        let old_tries = self.block_tries;
        let commit_block = if self.block_tries >= config::BLOCK_FORMATION_TRIES {
            self.block_tries = 0;
            true
        } else {
            // Try filling 4/5 of a block;
            if proposed_block.min_chunks() >= 4 * block_size_chunks() / 5 {
                self.block_tries = 0;
                true
            } else {
                self.block_tries += 1;
                false
            }
        };

        if commit_block {
            debug!("Creating block, tries {}", old_tries);

            self.statekeeper_requests
                .send(StateKeeperRequest::ExecuteBlock(proposed_block))
                .await;
        }
    }
}

// driving engine of the application
pub fn run_block_proposer_task(
    mut mempool_requests: mpsc::Sender<MempoolRequest>,
    mut statekeeper_requests: mpsc::Sender<StateKeeperRequest>,
    runtime: &Runtime,
) {
    // TODO: proper const

    runtime.spawn(async move {
        let mut timer = time::interval(Duration::from_secs(5));

        let last_unprocessed_prior_op_chan = oneshot::channel();
        statekeeper_requests
            .send(StateKeeperRequest::GetLastUnprocessedPriorityOp(
                last_unprocessed_prior_op_chan.0,
            ))
            .await;
        let current_priority_op_number = last_unprocessed_prior_op_chan
            .1
            .await
            .expect("Unprocessed priority op initialization");

        let mut block_proposer = BlockProposer {
            current_priority_op_number,
            block_tries: 0,
            mempool_requests,
            statekeeper_requests,
        };

        loop {
            timer.tick().await;

            block_proposer.commit_new_block_or_wait_for_txs().await;
        }
    });
}
