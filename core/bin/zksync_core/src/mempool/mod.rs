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
use std::{collections::HashMap, sync::Arc};
// External uses
use futures::{
    channel::{
        mpsc::{self, Receiver},
        oneshot,
    },
    SinkExt, StreamExt,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

// Workspace uses
use zksync_balancer::{Balancer, BuildBalancedItem};
use zksync_config::ZkSyncConfig;
use zksync_storage::ConnectionPool;
use zksync_types::{
    mempool::{SignedTxVariant, SignedTxsBatch},
    tx::TxEthSignature,
    AccountId, AccountUpdate, AccountUpdates, Address, Nonce, PriorityOp, SignedZkSyncTx,
    TransferOp, TransferToNewOp, ZkSyncTx,
};

// Local uses
use crate::mempool::mempool_transactions_queue::MempoolTransactionsQueue;
use crate::{eth_watch::EthWatchRequest, wait_for_tasks};

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
    pub fn is_empty(&self) -> bool {
        self.priority_ops.is_empty() && self.txs.is_empty()
    }
}

#[derive(Debug)]
pub struct GetBlockRequest {
    pub last_priority_op_number: u64,
    pub block_timestamp: u64,
    pub response_sender: oneshot::Sender<ProposedBlock>,
}

#[derive(Debug)]
pub enum MempoolTransactionRequest {
    /// Add new transaction to mempool, transaction should be previously checked
    /// for correctness (including its Ethereum and ZKSync signatures).
    /// oneshot is used to receive tx add result.
    NewTx(Box<SignedZkSyncTx>, oneshot::Sender<Result<(), TxAddError>>),
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
    /// When block is committed, nonces of the account tree should be updated too.
    UpdateNonces(AccountUpdates),
    /// Get transactions from the mempool.
    GetBlock(GetBlockRequest),
}

struct MempoolState {
    // account and last committed nonce
    account_nonces: HashMap<Address, Nonce>,
    account_ids: HashMap<AccountId, Address>,
    transactions_queue: MempoolTransactionsQueue,
}

impl MempoolState {
    fn chunks_for_tx(&self, tx: &ZkSyncTx) -> usize {
        match tx {
            ZkSyncTx::Transfer(tx) => {
                if self.account_nonces.contains_key(&tx.to) {
                    TransferOp::CHUNKS
                } else {
                    TransferToNewOp::CHUNKS
                }
            }
            _ => tx.min_chunks(),
        }
    }

    fn chunks_for_batch(&self, batch: &SignedTxsBatch) -> usize {
        batch.txs.iter().map(|tx| self.chunks_for_tx(&tx.tx)).sum()
    }

    fn required_chunks(&self, element: &SignedTxVariant) -> usize {
        match element {
            SignedTxVariant::Tx(tx) => self.chunks_for_tx(&tx.tx),
            SignedTxVariant::Batch(batch) => self.chunks_for_batch(batch),
        }
    }

    async fn restore_from_db(db_pool: &ConnectionPool) -> Self {
        let mut storage = db_pool.access_storage().await.expect("mempool db restore");
        let mut transaction = storage
            .start_transaction()
            .await
            .expect("mempool db transaction");

        let (_, accounts) = transaction
            .chain()
            .state_schema()
            .load_committed_state(None)
            .await
            .expect("mempool account state load");

        let mut account_ids = HashMap::new();
        let mut account_nonces = HashMap::new();

        for (id, account) in accounts {
            account_ids.insert(id, account.address);
            account_nonces.insert(account.address, account.nonce);
        }

        // Remove any possible duplicates of already executed transactions
        // from the database.
        transaction
            .chain()
            .mempool_schema()
            .collect_garbage()
            .await
            .expect("Collecting garbage in the mempool schema failed");

        // Load transactions that were not yet processed and are awaiting in the
        // mempool.
        let all_mempool_txs = transaction
            .chain()
            .mempool_schema()
            .load_txs()
            .await
            .expect("Attempt to restore mempool txs from DB failed");

        // Transactions can become ready when knowing the block timestamp
        let mut transactions_queue = MempoolTransactionsQueue::new();

        for tx in all_mempool_txs.clone() {
            transactions_queue.add_tx_variant(tx);
        }

        transaction
            .commit()
            .await
            .expect("mempool db transaction commit");

        vlog::info!(
            "{} transactions were restored from the persistent mempool storage",
            all_mempool_txs.len()
        );

        Self {
            account_nonces,
            account_ids,
            transactions_queue,
        }
    }

    fn nonce(&self, address: &Address) -> Nonce {
        *self.account_nonces.get(address).unwrap_or(&Nonce(0))
    }

    fn add_tx(&mut self, tx: SignedZkSyncTx) {
        self.transactions_queue.add_tx_variant(tx.into());
    }

    fn add_batch(&mut self, batch: SignedTxsBatch) {
        assert_ne!(batch.batch_id, 0, "Batch ID was not set");

        self.transactions_queue
            .add_tx_variant(SignedTxVariant::Batch(batch));
    }
}

struct MempoolBlocksHandler {
    mempool_state: Arc<RwLock<MempoolState>>,
    requests: mpsc::Receiver<MempoolBlocksRequest>,
    eth_watch_req: mpsc::Sender<EthWatchRequest>,
    max_block_size_chunks: usize,
}

impl MempoolBlocksHandler {
    async fn propose_new_block(
        &mut self,
        current_unprocessed_priority_op: u64,
        block_timestamp: u64,
    ) -> ProposedBlock {
        let start = std::time::Instant::now();
        let (chunks_left, priority_ops) = self
            .select_priority_ops(current_unprocessed_priority_op)
            .await;
        let (_chunks_left, txs) = self
            .prepare_tx_for_block(chunks_left, block_timestamp)
            .await;

        if !priority_ops.is_empty() {
            vlog::debug!("Proposed priority ops for block: {:?}", priority_ops);
        }
        if !txs.is_empty() {
            vlog::debug!("Proposed txs for block: {:?}", txs);
        }
        metrics::histogram!("mempool.propose_new_block", start.elapsed());
        ProposedBlock { priority_ops, txs }
    }

    /// Returns: chunks left from max amount of chunks, ops selected
    async fn select_priority_ops(
        &self,
        current_unprocessed_priority_op: u64,
    ) -> (usize, Vec<PriorityOp>) {
        let (sender, receiver) = oneshot::channel();
        self.eth_watch_req
            .clone()
            .send(EthWatchRequest::GetPriorityQueueOps {
                op_start_id: current_unprocessed_priority_op,
                max_chunks: self.max_block_size_chunks,
                resp: sender,
            })
            .await
            .expect("ETH watch req receiver dropped");

        let priority_ops = receiver.await.expect("Err response from eth watch");

        (
            self.max_block_size_chunks
                - priority_ops
                    .iter()
                    .map(|op| op.data.chunks())
                    .sum::<usize>(),
            priority_ops,
        )
    }

    async fn prepare_tx_for_block(
        &mut self,
        mut chunks_left: usize,
        block_timestamp: u64,
    ) -> (usize, Vec<SignedTxVariant>) {
        let mut mempool_state = self.mempool_state.write().await;

        mempool_state
            .transactions_queue
            .prepare_new_ready_transactions(block_timestamp);

        let mut txs_for_commit = Vec::new();

        while let Some(tx) = mempool_state.transactions_queue.pop_front() {
            let chunks_for_tx = mempool_state.required_chunks(&tx);
            if chunks_left >= chunks_for_tx {
                txs_for_commit.push(tx);
                chunks_left -= chunks_for_tx;
            } else {
                // Push the taken tx back, it does not fit.
                mempool_state.transactions_queue.push_front(tx);
                break;
            }
        }

        (chunks_left, txs_for_commit)
    }

    async fn run(mut self) {
        vlog::info!("Block mempool handler is running");
        while let Some(request) = self.requests.next().await {
            match request {
                MempoolBlocksRequest::GetBlock(block) => {
                    // Generate proposed block.
                    let proposed_block = self
                        .propose_new_block(block.last_priority_op_number, block.block_timestamp)
                        .await;

                    // Send the proposed block to the request initiator.
                    block
                        .response_sender
                        .send(proposed_block)
                        .expect("mempool proposed block response send failed");
                }
                MempoolBlocksRequest::UpdateNonces(updates) => {
                    for (id, update) in updates {
                        match update {
                            AccountUpdate::Create { address, nonce } => {
                                let mut mempool = self.mempool_state.write().await;
                                mempool.account_ids.insert(id, address);
                                mempool.account_nonces.insert(address, nonce);
                            }
                            AccountUpdate::Delete { address, .. } => {
                                let mut mempool = self.mempool_state.write().await;
                                mempool.account_ids.remove(&id);
                                mempool.account_nonces.remove(&address);
                            }
                            AccountUpdate::UpdateBalance { new_nonce, .. } => {
                                let address = self
                                    .mempool_state
                                    .read()
                                    .await
                                    .account_ids
                                    .get(&id)
                                    .cloned();
                                if let Some(address) = address {
                                    if let Some(nonce) = self
                                        .mempool_state
                                        .write()
                                        .await
                                        .account_nonces
                                        .get_mut(&address)
                                    {
                                        *nonce = new_nonce;
                                    }
                                }
                            }
                            AccountUpdate::ChangePubKeyHash { new_nonce, .. } => {
                                let address = self
                                    .mempool_state
                                    .read()
                                    .await
                                    .account_ids
                                    .get(&id)
                                    .cloned();

                                if let Some(address) = address {
                                    if let Some(nonce) = self
                                        .mempool_state
                                        .write()
                                        .await
                                        .account_nonces
                                        .get_mut(&address)
                                    {
                                        *nonce = new_nonce;
                                    }
                                }
                            }
                            AccountUpdate::MintNFT { .. } | AccountUpdate::RemoveNFT { .. } => {
                                // Minting nft affects only tokens, mempool doesn't contain them
                            }
                        }
                    }
                }
            }
        }
    }
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
        if tx.nonce() < self.mempool_state.read().await.nonce(&tx.account()) {
            return Err(TxAddError::NonceMismatch);
        }

        let mut storage = self.db_pool.access_storage().await.map_err(|err| {
            vlog::warn!("Mempool storage access error: {}", err);
            TxAddError::DbError
        })?;

        storage
            .chain()
            .mempool_schema()
            .insert_tx(&tx)
            .await
            .map_err(|err| {
                vlog::warn!("Mempool storage access error: {}", err);
                TxAddError::DbError
            })?;

        self.mempool_state.write().await.add_tx(tx);
        Ok(())
    }

    async fn add_batch(
        &mut self,
        txs: Vec<SignedZkSyncTx>,
        eth_signatures: Vec<TxEthSignature>,
    ) -> Result<(), TxAddError> {
        for tx in txs.iter() {
            // Correctness should be checked by `signature_checker`, thus
            // `tx.check_correctness()` is not invoked here.
            if tx.nonce() < self.mempool_state.read().await.nonce(&tx.account()) {
                return Err(TxAddError::NonceMismatch);
            }
        }

        let mut batch: SignedTxsBatch = SignedTxsBatch {
            txs: txs.clone(),
            batch_id: 0, // Will be determined after inserting to the database
            eth_signatures: eth_signatures.clone(),
        };

        if self.mempool_state.read().await.chunks_for_batch(&batch) > self.max_block_size_chunks {
            return Err(TxAddError::BatchTooBig);
        }

        let mut storage = self.db_pool.access_storage().await.map_err(|err| {
            vlog::warn!("Mempool storage access error: {}", err);
            TxAddError::DbError
        })?;

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

        self.mempool_state.write().await.add_batch(batch);
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
            }
        }
    }
}

#[must_use]
pub fn run_mempool_tasks(
    db_pool: ConnectionPool,
    tx_requests: mpsc::Receiver<MempoolTransactionRequest>,
    block_requests: mpsc::Receiver<MempoolBlocksRequest>,
    eth_watch_req: mpsc::Sender<EthWatchRequest>,
    config: &ZkSyncConfig,
    number_of_mempool_transaction_handlers: u8,
    channel_capacity: usize,
) -> JoinHandle<()> {
    let config = config.clone();
    tokio::spawn(async move {
        let mempool_state = Arc::new(RwLock::new(MempoolState::restore_from_db(&db_pool).await));
        let max_block_size_chunks = *config
            .chain
            .state_keeper
            .block_chunk_sizes
            .iter()
            .max()
            .expect("failed to find max block chunks size");
        let mut tasks = vec![];
        let (balancer, handlers) = Balancer::new(
            MempoolTransactionsHandlerBuilder {
                db_pool: db_pool.clone(),
                mempool_state: mempool_state.clone(),
                max_block_size_chunks,
            },
            tx_requests,
            number_of_mempool_transaction_handlers,
            channel_capacity,
        );

        for item in handlers.into_iter() {
            tasks.push(tokio::spawn(item.run()));
        }

        tasks.push(tokio::spawn(balancer.run()));

        let blocks_handler = MempoolBlocksHandler {
            mempool_state,
            requests: block_requests,
            eth_watch_req,
            max_block_size_chunks,
        };
        tasks.push(tokio::spawn(blocks_handler.run()));
        wait_for_tasks(tasks).await
    })
}
