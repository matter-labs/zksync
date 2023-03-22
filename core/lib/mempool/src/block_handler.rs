use futures::channel::{mpsc, oneshot};
use futures::StreamExt;

use zksync_types::{
    mempool::SignedTxVariant,
    tx::{error::TxAddError, TxHash},
    PriorityOp,
};

use crate::state::MempoolState;

#[derive(Clone, Debug, Default)]
pub struct ProposedBlock {
    pub priority_ops: Vec<PriorityOp>,
    pub txs: Vec<SignedTxVariant>,
}

impl ProposedBlock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.priority_ops.is_empty() && self.txs.is_empty()
    }

    pub fn size(&self) -> usize {
        self.priority_ops.len() + self.txs.len()
    }
}

#[derive(Debug)]
pub struct GetBlockRequest {
    pub last_priority_op_number: u64,
    pub block_timestamp: u64,
    pub chunks_left: usize,
    pub executed_txs: Vec<TxHash>,
    pub response_sender: oneshot::Sender<ProposedBlock>,
}

#[derive(Debug)]
pub enum MempoolBlocksRequest {
    /// Get transactions from the mempool.
    GetBlock(GetBlockRequest),
}

pub(crate) struct MempoolBlocksHandler {
    pub mempool_state: MempoolState,
    pub requests: mpsc::Receiver<MempoolBlocksRequest>,
}

impl MempoolBlocksHandler {
    async fn propose_new_block(
        &mut self,
        current_unprocessed_priority_op: u64,
        block_timestamp: u64,
        chunks_left: usize,
        executed_txs: &[TxHash],
    ) -> Result<ProposedBlock, TxAddError> {
        let start = std::time::Instant::now();
        // Try to exhaust the reverted transactions queue. Most of the time it
        // will be empty unless the server is restarted after reverting blocks.
        let mut tx_queue = self
            .mempool_state
            .get_transaction_queue(executed_txs)
            .await?;

        let (txs, priority_ops, chunks_left) = tx_queue
            .select_transactions(
                chunks_left,
                current_unprocessed_priority_op,
                block_timestamp,
                &self.mempool_state,
            )
            .await?;

        if !priority_ops.is_empty() || !txs.is_empty() {
            vlog::debug!(
                "Proposed {} priority ops and {} txs for the next miniblock; {} chunks left",
                priority_ops.len(),
                txs.len(),
                chunks_left
            );
        }

        metrics::histogram!("mempool.propose_new_block", start.elapsed());

        for pr_op in &priority_ops {
            let labels = vec![
                ("stage", "propose_block".to_string()),
                ("name", pr_op.data.variance_name()),
                ("token", pr_op.data.token_id().to_string()),
            ];

            metrics::increment_counter!("process_tx_count", &labels)
        }

        for tx_variant in &txs {
            for tx in tx_variant.get_transactions() {
                let labels = vec![
                    ("stage", "propose_block".to_string()),
                    ("name", tx.tx.variance_name()),
                    ("token", tx.tx.token_id().to_string()),
                ];
                metrics::histogram!("process_tx", tx.elapsed(), &labels);
            }
        }
        Ok(ProposedBlock { priority_ops, txs })
    }

    pub async fn run(mut self) {
        vlog::info!("Block mempool handler is running");
        // We have to clean garbage from mempool before running the block generator.
        // Remove any possible duplicates of already executed transactions
        // from the database.
        self.mempool_state.collect_garbage().await;
        while let Some(request) = self.requests.next().await {
            match request {
                MempoolBlocksRequest::GetBlock(block) => {
                    // Generate proposed block.
                    let proposed_block = self
                        .propose_new_block(
                            block.last_priority_op_number,
                            block.block_timestamp,
                            block.chunks_left,
                            &block.executed_txs,
                        )
                        .await
                        .expect("Unable to propose the new miniblock");

                    // Send the proposed block to the request initiator.
                    block
                        .response_sender
                        .send(proposed_block)
                        .expect("Mempool request receiver dropped");
                }
            }
        }
    }
}
