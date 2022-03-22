//! Mempool is buffer for transactions.
//!
//! The role is:
//! 1) Storing txs to the database
//! 2) Getting txs from database.
//! 3) When polled return vector of the transactions in the queue.
//!
//! For better consistency, we always store all txs in the database and get them only if they are requested.
//!
//! Communication channel with other actors:
//! Mempool does not push information to other actors, only accepts requests. (see `MempoolRequest`)

use std::collections::HashSet;
use std::time::Instant;
// External uses
use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};

use tokio::task::JoinHandle;

// Workspace uses
use zksync_storage::{ConnectionPool, StorageProcessor};
use zksync_types::tx::error::TxAddError;
use zksync_types::tx::TxHash;
use zksync_types::{
    mempool::{SignedTxVariant, SignedTxsBatch},
    tx::TxEthSignature,
    Address, PriorityOp, SignedZkSyncTx, TransferOp, TransferToNewOp, ZkSyncTx,
};

// Local uses
use crate::mempool_transactions_queue::MempoolTransactionsQueue;

mod mempool_transactions_queue;

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
}

#[derive(Debug)]
pub struct GetBlockRequest {
    pub last_priority_op_number: u64,
    pub block_timestamp: u64,
    pub executed_txs: Vec<TxHash>,
    pub response_sender: oneshot::Sender<ProposedBlock>,
}

#[derive(Debug)]
pub enum MempoolTransactionRequest {
    /// Add new transaction to mempool, transaction should be previously checked
    /// for correctness (including its Ethereum and ZKSync signatures).
    /// oneshot is used to receive tx add result.
    NewTx(Box<SignedZkSyncTx>, oneshot::Sender<Result<(), TxAddError>>),

    /// Add new priority ops, confirmed or not
    NewPriorityOps(
        Vec<PriorityOp>,
        bool,
        oneshot::Sender<Result<(), TxAddError>>,
    ),
    /// Add a new batch of transactions to the mempool. All transactions in batch must
    /// be either executed successfully, or otherwise fail all together.
    /// Invariants for each individual transaction in the batch are the same as in
    /// `NewTx` variant of this enum.
    NewTxsBatch(
        Vec<SignedZkSyncTx>,
        Vec<TxEthSignature>,
        oneshot::Sender<Result<(), TxAddError>>,
    ),
}

#[derive(Debug)]
pub enum MempoolBlocksRequest {
    /// Get transactions from the mempool.
    GetBlock(GetBlockRequest),
}

#[derive(Debug, Clone)]
struct MempoolState {
    db_pool: ConnectionPool,
}

impl MempoolState {
    async fn chunks_for_tx_with_cache(
        &self,
        tx: &ZkSyncTx,
        storage: &mut StorageProcessor<'_>,
        existed_accounts: &mut HashSet<Address>,
    ) -> Result<usize, TxAddError> {
        let start = Instant::now();
        let res = match tx {
            ZkSyncTx::Transfer(tx) => {
                let exist = if existed_accounts.contains(&tx.to) {
                    true
                } else {
                    storage
                        .chain()
                        .account_schema()
                        .does_account_exist(tx.to)
                        .await
                        .map_err(|_| TxAddError::DbError)?
                };
                if exist {
                    existed_accounts.insert(tx.to);
                    TransferOp::CHUNKS
                } else {
                    TransferToNewOp::CHUNKS
                }
            }
            _ => tx.min_chunks(),
        };
        metrics::histogram!("mempool_state.chunks_for_tx", start.elapsed(), "type" => "with_cache");
        Ok(res)
    }
    async fn chunks_for_tx(&self, tx: &ZkSyncTx) -> Result<usize, TxAddError> {
        let start = Instant::now();
        let res = match tx {
            ZkSyncTx::Transfer(tx) => {
                let exist = self
                    .db_pool
                    .access_storage()
                    .await
                    .map_err(|_| TxAddError::DbError)?
                    .chain()
                    .account_schema()
                    .does_account_exist(tx.to)
                    .await
                    .map_err(|_| TxAddError::DbError)?;
                if exist {
                    TransferOp::CHUNKS
                } else {
                    TransferToNewOp::CHUNKS
                }
            }
            _ => tx.min_chunks(),
        };
        metrics::histogram!("mempool_state.chunks_for_tx", start.elapsed(), "type" => "without_cache");
        Ok(res)
    }

    async fn chunks_for_batch(&self, batch: &SignedTxsBatch) -> Result<usize, TxAddError> {
        let start = Instant::now();
        let mut size = 0;
        let mut storage = self
            .db_pool
            .access_storage()
            .await
            .map_err(|_| TxAddError::DbError)?;
        let mut existed_accounts = HashSet::new();
        for tx in &batch.txs {
            size += self
                .chunks_for_tx_with_cache(&tx.tx, &mut storage, &mut existed_accounts)
                .await?;
        }
        metrics::histogram!("mempool_state.chunks_for_batch", start.elapsed());
        Ok(size)
    }

    async fn required_chunks(&self, element: &SignedTxVariant) -> Result<usize, TxAddError> {
        match element {
            SignedTxVariant::Tx(tx) => self.chunks_for_tx(&tx.tx).await,
            SignedTxVariant::Batch(batch) => self.chunks_for_batch(batch).await,
        }
    }

    async fn collect_garbage(&self) {
        let mut storage = self.db_pool.access_storage().await.expect("Db error");
        // Remove any possible duplicates of already executed transactions
        // from the database.
        storage
            .chain()
            .mempool_schema()
            .collect_garbage()
            .await
            .expect("Db error");
    }

    pub fn new(db_pool: ConnectionPool) -> Self {
        Self { db_pool }
    }

    async fn get_transaction_queue(
        &self,
        executed_txs: &[TxHash],
    ) -> Result<MempoolTransactionsQueue, TxAddError> {
        let mut storage = self
            .db_pool
            .access_storage()
            .await
            .map_err(|_| TxAddError::DbError)?;
        let mut transaction = storage
            .start_transaction()
            .await
            .map_err(|_| TxAddError::DbError)?;

        let priority_ops = transaction
            .chain()
            .mempool_schema()
            .get_confirmed_priority_ops()
            .await
            .map_err(|_| TxAddError::DbError)?;

        // Load transactions that were not yet processed and are awaiting in the
        // mempool.
        let mempool_txs = transaction
            .chain()
            .mempool_schema()
            .load_txs(executed_txs)
            .await
            .map_err(|_| TxAddError::DbError)?;

        let transactions_queue = MempoolTransactionsQueue::new(priority_ops, mempool_txs);

        Ok(transactions_queue)
    }
}

struct MempoolBlocksHandler {
    mempool_state: MempoolState,
    requests: mpsc::Receiver<MempoolBlocksRequest>,
    max_block_size_chunks: usize,
}

impl MempoolBlocksHandler {
    async fn propose_new_block(
        &mut self,
        current_unprocessed_priority_op: u64,
        block_timestamp: u64,
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
                self.max_block_size_chunks,
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

    async fn run(mut self) {
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

struct MempoolTransactionsHandler {
    db_pool: ConnectionPool,
    mempool_state: MempoolState,
    requests: mpsc::Receiver<MempoolTransactionRequest>,
    max_block_size_chunks: usize,
}

impl MempoolTransactionsHandler {
    async fn add_tx(&mut self, tx: SignedZkSyncTx) -> Result<(), TxAddError> {
        // Correctness should be checked by `signature_checker`, thus
        // `tx.check_correctness()` is not invoked here.
        let mut storage = self.db_pool.access_storage().await.map_err(|err| {
            vlog::error!("Mempool storage access error: {}", err);
            TxAddError::DbError
        })?;

        let nonce = storage
            .chain()
            .account_schema()
            // Close operation does not exist so we will never met this error
            .estimate_nonce(tx.account_id().map_err(|_| TxAddError::Other)?)
            .await
            .map_err(|_| TxAddError::DbError)?
            .unwrap_or_default();

        if tx.nonce() < nonce {
            return Err(TxAddError::NonceMismatch);
        }

        storage
            .chain()
            .mempool_schema()
            .insert_tx(&tx)
            .await
            .map_err(|err| {
                vlog::error!("Mempool storage access error: {}", err);
                TxAddError::DbError
            })?;

        let labels = vec![
            ("stage", "mempool".to_string()),
            ("name", tx.tx.variance_name()),
            ("token", tx.tx.token_id().to_string()),
        ];
        metrics::histogram!("process_tx", tx.elapsed(), &labels);

        Ok(())
    }

    /// Add priority operations to the mempool. For a better UX, we save unconfirmed transactions
    /// to the database. And we will move them to the real queue when they are confirmed.
    async fn add_priority_ops(
        &mut self,
        mut ops: Vec<PriorityOp>,
        confirmed: bool,
    ) -> Result<(), TxAddError> {
        let mut storage = self.db_pool.access_storage().await.map_err(|err| {
            vlog::error!("Mempool storage access error: {}", err);
            TxAddError::DbError
        })?;
        let last_processed_priority_op = storage
            .chain()
            .operations_schema()
            .get_max_priority_op_serial_id()
            .await
            .map_err(|_| TxAddError::DbError)?;

        if let Some(serial_id) = last_processed_priority_op {
            ops.retain(|op| op.serial_id > serial_id)
        }

        // Nothing to insert
        if ops.is_empty() {
            return Ok(());
        }
        storage
            .chain()
            .mempool_schema()
            .insert_priority_ops(&ops, confirmed)
            .await
            .map_err(|err| {
                vlog::error!("Mempool storage access error: {}", err);
                TxAddError::DbError
            })?;

        for op in &ops {
            let labels = vec![
                ("stage", "mempool".to_string()),
                ("name", op.data.variance_name()),
                ("token", op.data.token_id().to_string()),
            ];
            metrics::increment_counter!("process_tx_count", &labels);
        }

        Ok(())
    }

    async fn add_batch(
        &mut self,
        txs: Vec<SignedZkSyncTx>,
        eth_signatures: Vec<TxEthSignature>,
    ) -> Result<(), TxAddError> {
        let batch: SignedTxsBatch = SignedTxsBatch {
            txs: txs.clone(),
            batch_id: 0, // Will be determined after inserting to the database
            eth_signatures: eth_signatures.clone(),
        };

        let mut storage = self.db_pool.access_storage().await.map_err(|err| {
            vlog::error!("Mempool storage access error: {}", err);
            TxAddError::DbError
        })?;

        for tx in txs.iter() {
            // Correctness should be checked by `signature_checker`, thus
            // `tx.check_correctness()` is not invoked here.
            let nonce = storage
                .chain()
                .account_schema()
                // Close operation does not exist so we will never met this error
                .estimate_nonce(tx.account_id().map_err(|_| TxAddError::Other)?)
                .await
                .map_err(|_| TxAddError::DbError)?
                .unwrap_or_default();

            if tx.nonce() < nonce {
                return Err(TxAddError::NonceMismatch);
            }
        }

        if self.mempool_state.chunks_for_batch(&batch).await? > self.max_block_size_chunks {
            return Err(TxAddError::BatchTooBig);
        }

        for tx in &batch.txs {
            let labels = vec![
                ("stage", "mempool".to_string()),
                ("name", tx.tx.variance_name()),
                ("token", tx.tx.token_id().to_string()),
            ];

            metrics::histogram!("process_tx", tx.elapsed(), &labels);
        }

        storage
            .chain()
            .mempool_schema()
            .insert_batch(&batch.txs, eth_signatures)
            .await
            .map_err(|err| {
                vlog::warn!("Mempool storage access error: {}", err);
                TxAddError::DbError
            })?;

        Ok(())
    }

    async fn run(mut self) {
        vlog::info!("Transaction mempool handler is running");
        while let Some(request) = self.requests.next().await {
            match request {
                MempoolTransactionRequest::NewTx(tx, resp) => {
                    let tx_add_result = self.add_tx(*tx).await;
                    resp.send(tx_add_result).unwrap_or_default();
                }
                MempoolTransactionRequest::NewTxsBatch(txs, eth_signatures, resp) => {
                    let tx_add_result = self.add_batch(txs, eth_signatures).await;
                    resp.send(tx_add_result).unwrap_or_default();
                }
                MempoolTransactionRequest::NewPriorityOps(ops, confirmed, resp) => {
                    let tx_add_result = self.add_priority_ops(ops, confirmed).await;
                    resp.send(tx_add_result).unwrap_or_default();
                }
            }
        }
    }
}

// Due channel based nature, for better performance,
// you need to run independent mempool_tx_handler for each actor, e.g. for each API actor
#[must_use]
pub fn run_mempool_tx_handler(
    db_pool: ConnectionPool,
    tx_requests: mpsc::Receiver<MempoolTransactionRequest>,
    block_chunk_sizes: Vec<usize>,
) -> JoinHandle<()> {
    let mempool_state = MempoolState::new(db_pool.clone());
    let max_block_size_chunks = *block_chunk_sizes
        .iter()
        .max()
        .expect("failed to find max block chunks size");
    let handler = MempoolTransactionsHandler {
        db_pool,
        mempool_state,
        requests: tx_requests,
        max_block_size_chunks,
    };
    tokio::spawn(handler.run())
}

pub fn run_mempool_block_handler(
    db_pool: ConnectionPool,
    block_requests: mpsc::Receiver<MempoolBlocksRequest>,
    block_chunk_sizes: Vec<usize>,
) -> JoinHandle<()> {
    let mempool_state = MempoolState::new(db_pool);
    let max_block_size_chunks = *block_chunk_sizes
        .iter()
        .max()
        .expect("failed to find max block chunks size");

    let blocks_handler = MempoolBlocksHandler {
        mempool_state,
        requests: block_requests,
        max_block_size_chunks,
    };

    tokio::spawn(blocks_handler.run())
}
