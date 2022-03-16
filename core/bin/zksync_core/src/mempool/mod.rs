//! Mempool is simple in memory buffer for transactions.
//!
//! Its role is to:
//! 1) Accept transactions from api, check signatures and basic nonce correctness(nonce not too small).
//! To do nonce correctness check mempool stores mapping `AccountAddress -> Nonce`, this mapping is updated
//! when new block is committed.
//! 2) When polled return vector of the transactions in the queue.
//!
//! Mempool is not persisted on disc, all transactions will be lost on node shutdown.
//!
//! Communication channel with other actors:
//! Mempool does not push information to other actors, only accepts requests. (see `MempoolRequest`)
//!
//! Communication with db:
//! on restart mempool restores nonces of the accounts that are stored in the account tree.

// Built-in deps
use std::{iter, sync::Arc};
// External uses
use futures::{
    channel::{
        mpsc::{self, Receiver},
        oneshot,
    },
    StreamExt,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use web3::types::H256;

// Workspace uses
use zksync_balancer::BuildBalancedItem;

use zksync_storage::ConnectionPool;
use zksync_types::tx::TxHash;
use zksync_types::{
    mempool::{SignedTxVariant, SignedTxsBatch},
    tx::TxEthSignature,
    PriorityOp, SignedZkSyncTx, TransferOp, TransferToNewOp, ZkSyncTx,
};

// Local uses
use crate::mempool::mempool_transactions_queue::MempoolTransactionsQueue;
use crate::wait_for_tasks;

mod mempool_transactions_queue;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Error)]
pub enum TxAddError {
    #[error("Tx nonce is too low.")]
    NonceMismatch,

    #[error("Tx is incorrect")]
    IncorrectTx,

    #[error("Transaction fee is too low")]
    TxFeeTooLow,

    #[error("Transactions batch summary fee is too low")]
    TxBatchFeeTooLow,

    #[error("EIP1271 signature could not be verified")]
    EIP1271SignatureVerificationFail,

    #[error("MissingEthSignature")]
    MissingEthSignature,

    #[error("Eth signature is incorrect")]
    IncorrectEthSignature,

    #[error("Change pubkey tx is not authorized onchain")]
    ChangePkNotAuthorized,

    #[error("Internal error")]
    Other,

    #[error("Database unavailable")]
    DbError,

    #[error("Transaction batch is empty")]
    EmptyBatch,

    #[error("Batch will not fit in any of supported block sizes")]
    BatchTooBig,

    #[error("The number of withdrawals in the batch is too big")]
    BatchWithdrawalsOverload,
}

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

struct MempoolState {
    db_pool: ConnectionPool,
    // transactions_queue: MempoolTransactionsQueue,
}

impl MempoolState {
    async fn chunks_for_tx(&self, tx: &ZkSyncTx) -> Result<usize, TxAddError> {
        Ok(match tx {
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
        })
    }

    async fn chunks_for_batch(&self, batch: &SignedTxsBatch) -> Result<usize, TxAddError> {
        let mut size = 0;
        for tx in &batch.txs {
            size += self.chunks_for_tx(&tx.tx).await?;
        }
        Ok(size)
    }

    async fn required_chunks(&self, element: &SignedTxVariant) -> Result<usize, TxAddError> {
        match element {
            SignedTxVariant::Tx(tx) => self.chunks_for_tx(&tx.tx).await,
            SignedTxVariant::Batch(batch) => self.chunks_for_batch(batch).await,
        }
    }

    pub fn new(db_pool: ConnectionPool) -> Self {
        Self { db_pool }
    }

    async fn get_transaction_queue(&self, executed_txs: &[TxHash]) -> MempoolTransactionsQueue {
        let mut storage = self
            .db_pool
            .access_storage()
            .await
            .expect("mempool db restore");
        let mut transaction = storage
            .start_transaction()
            .await
            .expect("mempool db transaction");

        let priority_ops = transaction
            .chain()
            .mempool_schema()
            .get_confirmed_priority_ops()
            .await
            .expect("Get priority ops failed");

        // Remove any possible duplicates of already executed transactions
        // from the database.
        //TODO move it to start somewhere
        transaction
            .chain()
            .mempool_schema()
            .collect_garbage()
            .await
            .expect("Collecting garbage in the mempool schema failed");

        // Load transactions that were not yet processed and are awaiting in the
        // mempool.
        let (mempool_txs, reverted_txs) = transaction
            .chain()
            .mempool_schema()
            .load_txs(executed_txs)
            .await
            .expect("Attempt to restore mempool txs from DB failed");

        // Initialize the queue with reverted transactions loaded from the database.
        let serial_id = transaction
            .chain()
            .operations_schema()
            .get_max_priority_op_serial_id()
            .await
            .expect("Error in getting last priority op");
        let mut transactions_queue = MempoolTransactionsQueue::new(reverted_txs, serial_id);

        transactions_queue.add_priority_ops(priority_ops);

        for tx in mempool_txs {
            transactions_queue.add_tx_variant(tx);
        }

        transaction
            .commit()
            .await
            .expect("mempool db transaction commit");

        transactions_queue
    }
}

struct MempoolBlocksHandler {
    mempool_state: Arc<RwLock<MempoolState>>,
    requests: mpsc::Receiver<MempoolBlocksRequest>,
    max_block_size_chunks: usize,
}

impl MempoolBlocksHandler {
    async fn select_reverted_operations(
        &mut self,
        current_unprocessed_priority_op: u64,
        transactions_queue: &mut MempoolTransactionsQueue,
    ) -> (usize, ProposedBlock) {
        let mut chunks_left = self.max_block_size_chunks;
        let mut proposed_block = ProposedBlock::new();
        let mut next_serial_id = current_unprocessed_priority_op;

        let mempool_state = self.mempool_state.write().await;
        // Peek into the reverted queue.
        let reverted_tx = match transactions_queue.reverted_queue_front() {
            Some(reverted_tx) => reverted_tx,
            None => return (chunks_left, proposed_block),
        };
        // First, fill the block with missing priority operations.
        // Unlike transactions, they are requested from the Eth watch.
        let next_op_id = reverted_tx.next_priority_op_id;
        while next_serial_id < next_op_id {
            // Find the necessary serial id and skip the already processed
            let priority_op = iter::from_fn(|| transactions_queue.pop_front_priority_op())
                .find(|op| op.serial_id == next_serial_id)
                .unwrap_or_else(|| {
                    panic!(
                        "Operation not found in the priority queue {}",
                        next_serial_id,
                    )
                });

            // If the operation doesn't fit, return the proposed block.
            if priority_op.data.chunks() <= chunks_left {
                chunks_left -= priority_op.data.chunks();
                proposed_block.priority_ops.push(priority_op);
                next_serial_id += 1;
            } else {
                return (chunks_left, proposed_block);
            }
        }

        while let Some(reverted_tx) = transactions_queue.reverted_queue_front() {
            // To prevent state keeper from executing priority operations
            // out of order, we finish the miniblock if the serial id counter
            // is greater than the current one. Such transactions will be included
            // on the next block proposer request.
            if next_serial_id < reverted_tx.next_priority_op_id {
                break;
            }
            // If the transaction fits into the block, pop it from the queue.
            // TODO do not unwrap
            let required_chunks = mempool_state
                .required_chunks(reverted_tx.as_ref())
                .await
                .unwrap();
            if required_chunks <= chunks_left {
                chunks_left -= required_chunks;
                let reverted_tx = transactions_queue.reverted_queue_pop_front().unwrap();
                proposed_block.txs.push(reverted_tx.into_inner());
            } else {
                break;
            }
        }

        (chunks_left, proposed_block)
    }

    async fn propose_new_block(
        &mut self,
        current_unprocessed_priority_op: u64,
        block_timestamp: u64,
        executed_txs: &[TxHash],
    ) -> ProposedBlock {
        let start = std::time::Instant::now();
        // Try to exhaust the reverted transactions queue. Most of the time it
        // will be empty unless the server is restarted after reverting blocks.
        let state = self.mempool_state.write().await;
        let mut tx_queue = state.get_transaction_queue(executed_txs).await;
        drop(state);
        // TODO remove it. We use another approach how to correctly revert blocks
        let (chunks_left, reverted_block) = self
            .select_reverted_operations(current_unprocessed_priority_op, &mut tx_queue)
            .await;
        if !reverted_block.is_empty() {
            vlog::debug!(
                "Proposing new block with reverted operations, chunks used: {}",
                self.max_block_size_chunks - chunks_left
            );
            return reverted_block;
        }

        let (chunks_left, priority_ops) = select_priority_ops(
            self.max_block_size_chunks,
            current_unprocessed_priority_op,
            &mut tx_queue,
        )
        .await;

        let state = self.mempool_state.write().await;
        let (_chunks_left, txs) =
            prepare_tx_for_block(chunks_left, block_timestamp, &mut tx_queue, &state).await;
        drop(state);

        if !priority_ops.is_empty() {
            vlog::debug!("Proposed priority ops for block: {:?}", priority_ops);
        }
        if !txs.is_empty() {
            vlog::debug!("Proposed txs for block: {:?}", txs);
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
        ProposedBlock { priority_ops, txs }
    }

    async fn run(mut self) {
        vlog::info!("Block mempool handler is running");
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
                        .await;

                    // Send the proposed block to the request initiator.
                    block
                        .response_sender
                        .send(proposed_block)
                        .expect("mempool proposed block response send failed");
                }
            }
        }
    }
}

/// Returns: chunks left from max amount of chunks, ops selected
async fn select_priority_ops(
    max_block_size_chunks: usize,
    current_unprocessed_priority_op: u64,
    transactions_queue: &mut MempoolTransactionsQueue,
) -> (usize, Vec<PriorityOp>) {
    let mut result = vec![];

    let mut used_chunks = 0;
    let mut current_priority_op = current_unprocessed_priority_op;
    while let Some(op) = transactions_queue.pop_front_priority_op() {
        // Since the transaction addition is asynchronous process and we are checking node many times,
        // We can find some already processed priority ops
        if op.serial_id < current_priority_op {
            vlog::warn!("Already processed priority op was found in queue");
            // We can skip already processed priority operations
            continue;
        }
        assert_eq!(
            current_priority_op, op.serial_id,
            "Wrong order for priority ops"
        );
        if used_chunks + op.data.chunks() <= max_block_size_chunks {
            used_chunks += op.data.chunks();
            result.push(op);
            current_priority_op += 1;
        } else {
            break;
        }
    }
    (max_block_size_chunks - used_chunks, result)
}
async fn prepare_tx_for_block(
    mut chunks_left: usize,
    block_timestamp: u64,
    transactions_queue: &mut MempoolTransactionsQueue,
    mempool_state: &MempoolState,
) -> (usize, Vec<SignedTxVariant>) {
    transactions_queue.prepare_new_ready_transactions(block_timestamp);

    let mut txs_for_commit = Vec::new();

    while let Some(tx) = transactions_queue.pop_front() {
        // TODO DO not unwrap
        let chunks_for_tx = mempool_state.required_chunks(&tx).await.unwrap();
        if chunks_left >= chunks_for_tx {
            txs_for_commit.push(tx);
            chunks_left -= chunks_for_tx;
        } else {
            break;
        }
    }

    (chunks_left, txs_for_commit)
}
struct MempoolTransactionsHandler {
    db_pool: ConnectionPool,
    mempool_state: Arc<RwLock<MempoolState>>,
    requests: mpsc::Receiver<MempoolTransactionRequest>,
    max_block_size_chunks: usize,
}

struct MempoolTransactionsHandlerBuilder {
    db_pool: ConnectionPool,
    mempool_state: Arc<RwLock<MempoolState>>,
    max_block_size_chunks: usize,
}

impl BuildBalancedItem<MempoolTransactionRequest, MempoolTransactionsHandler>
    for MempoolTransactionsHandlerBuilder
{
    fn build_with_receiver(
        &self,
        receiver: Receiver<MempoolTransactionRequest>,
    ) -> MempoolTransactionsHandler {
        MempoolTransactionsHandler {
            db_pool: self.db_pool.clone(),
            mempool_state: self.mempool_state.clone(),
            requests: receiver,
            max_block_size_chunks: self.max_block_size_chunks,
        }
    }
}

impl MempoolTransactionsHandler {
    async fn add_tx(&mut self, tx: SignedZkSyncTx) -> Result<(), TxAddError> {
        // Correctness should be checked by `signature_checker`, thus
        // `tx.check_correctness()` is not invoked here.
        let mut storage = self.db_pool.access_storage().await.map_err(|err| {
            vlog::error!("Mempool storage access error: {}", err);
            TxAddError::DbError
        })?;

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
        // self.mempool_state.write().await.add_tx(tx);

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

        // // Add to queue only confirmed priority operations
        // if confirmed {
        //     self.mempool_state.write().await.add_ops(ops);
        // }
        //
        Ok(())
    }

    async fn add_batch(
        &mut self,
        txs: Vec<SignedZkSyncTx>,
        eth_signatures: Vec<TxEthSignature>,
    ) -> Result<(), TxAddError> {
        let mut batch: SignedTxsBatch = SignedTxsBatch {
            txs: txs.clone(),
            batch_id: 0, // Will be determined after inserting to the database
            eth_signatures: eth_signatures.clone(),
        };

        let mut storage = self.db_pool.access_storage().await.map_err(|err| {
            vlog::error!("Mempool storage access error: {}", err);
            TxAddError::DbError
        })?;

        if self
            .mempool_state
            .read()
            .await
            .chunks_for_batch(&batch)
            .await?
            > self.max_block_size_chunks
        {
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

        let batch_id = storage
            .chain()
            .mempool_schema()
            .insert_batch(&batch.txs, eth_signatures)
            .await
            .map_err(|err| {
                vlog::warn!("Mempool storage access error: {}", err);
                TxAddError::DbError
            })?;

        batch.batch_id = batch_id;

        // self.mempool_state.write().await.add_batch(batch);
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

#[must_use]
pub fn run_mempool_tasks(
    db_pool: ConnectionPool,
    tx_requests: mpsc::Receiver<MempoolTransactionRequest>,
    block_requests: mpsc::Receiver<MempoolBlocksRequest>,
    block_chunk_sizes: Vec<usize>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mempool_state = Arc::new(RwLock::new(MempoolState::new(db_pool.clone())));
        let max_block_size_chunks = *block_chunk_sizes
            .iter()
            .max()
            .expect("failed to find max block chunks size");
        let handler = MempoolTransactionsHandler {
            db_pool: db_pool.clone(),
            mempool_state: mempool_state.clone(),
            requests: tx_requests,
            max_block_size_chunks,
        };

        let blocks_handler = MempoolBlocksHandler {
            mempool_state,
            requests: block_requests,
            max_block_size_chunks,
        };

        let tasks = vec![
            tokio::spawn(blocks_handler.run()),
            tokio::spawn(handler.run()),
        ];
        wait_for_tasks(tasks).await
    })
}
