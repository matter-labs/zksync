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
use std::collections::{HashMap, VecDeque};
// External uses
use futures::{
    channel::{mpsc, oneshot},
    SinkExt, StreamExt,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::task::JoinHandle;
// Workspace uses
use zksync_storage::ConnectionPool;
use zksync_types::{
    mempool::{SignedTxVariant, SignedTxsBatch},
    AccountId, AccountUpdate, AccountUpdates, Address, Nonce, PriorityOp, SignedZkSyncTx,
    TransferOp, TransferToNewOp, ZkSyncTx,
};
// Local uses
use crate::eth_watch::EthWatchRequest;
use zksync_config::ConfigurationOptions;

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
    pub response_sender: oneshot::Sender<ProposedBlock>,
}

#[derive(Debug)]
pub enum MempoolRequest {
    /// Add new transaction to mempool, transaction should be previously checked
    /// for correctness (including its Ethereum and ZKSync signatures).
    /// oneshot is used to receive tx add result.
    NewTx(Box<SignedZkSyncTx>, oneshot::Sender<Result<(), TxAddError>>),
    /// Add a new batch of transactions to the mempool. All transactions in batch must
    /// be either executed successfully, or otherwise fail all together.
    /// Invariants for each individual transaction in the batch are the same as in
    /// `NewTx` variant of this enum.
    NewTxsBatch(Vec<SignedZkSyncTx>, oneshot::Sender<Result<(), TxAddError>>),
    /// When block is committed, nonces of the account tree should be updated too.
    UpdateNonces(AccountUpdates),
    /// Get transactions from the mempool.
    GetBlock(GetBlockRequest),
}

struct MempoolState {
    // account and last committed nonce
    account_nonces: HashMap<Address, Nonce>,
    account_ids: HashMap<AccountId, Address>,
    ready_txs: VecDeque<SignedTxVariant>,
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
        let ready_txs: VecDeque<_> = transaction
            .chain()
            .mempool_schema()
            .load_txs()
            .await
            .expect("Attempt to restore mempool txs from DB failed");

        transaction
            .commit()
            .await
            .expect("mempool db transaction commit");

        log::info!(
            "{} transactions were restored from the persistent mempool storage",
            ready_txs.len()
        );

        Self {
            account_nonces,
            account_ids,
            ready_txs,
        }
    }

    fn nonce(&self, address: &Address) -> Nonce {
        *self.account_nonces.get(address).unwrap_or(&0)
    }

    fn add_tx(&mut self, tx: SignedZkSyncTx) -> Result<(), TxAddError> {
        // Correctness should be checked by `signature_checker`, thus
        // `tx.check_correctness()` is not invoked here.

        if tx.nonce() >= self.nonce(&tx.account()) {
            self.ready_txs.push_back(tx.into());
            Ok(())
        } else {
            Err(TxAddError::NonceMismatch)
        }
    }

    fn add_batch(&mut self, batch: SignedTxsBatch) -> Result<(), TxAddError> {
        assert_ne!(batch.batch_id, 0, "Batch ID was not set");

        for tx in batch.txs.iter() {
            if tx.nonce() < self.nonce(&tx.account()) {
                return Err(TxAddError::NonceMismatch);
            }
        }

        self.ready_txs.push_back(SignedTxVariant::Batch(batch));

        Ok(())
    }
}

struct Mempool {
    db_pool: ConnectionPool,
    mempool_state: MempoolState,
    requests: mpsc::Receiver<MempoolRequest>,
    eth_watch_req: mpsc::Sender<EthWatchRequest>,
    max_block_size_chunks: usize,
    max_number_of_withdrawals_per_block: usize,
}

impl Mempool {
    async fn add_tx(&mut self, tx: SignedZkSyncTx) -> Result<(), TxAddError> {
        let mut storage = self.db_pool.access_storage().await.map_err(|err| {
            log::warn!("Mempool storage access error: {}", err);
            TxAddError::DbError
        })?;

        let mut transaction = storage.start_transaction().await.map_err(|err| {
            log::warn!("Mempool storage access error: {}", err);
            TxAddError::DbError
        })?;
        transaction
            .chain()
            .mempool_schema()
            .insert_tx(&tx)
            .await
            .map_err(|err| {
                log::warn!("Mempool storage access error: {}", err);
                TxAddError::DbError
            })?;

        transaction.commit().await.map_err(|err| {
            log::warn!("Mempool storage access error: {}", err);
            TxAddError::DbError
        })?;

        self.mempool_state.add_tx(tx)
    }

    async fn add_batch(&mut self, txs: Vec<SignedZkSyncTx>) -> Result<(), TxAddError> {
        let mut storage = self.db_pool.access_storage().await.map_err(|err| {
            log::warn!("Mempool storage access error: {}", err);
            TxAddError::DbError
        })?;

        let mut batch: SignedTxsBatch = SignedTxsBatch {
            txs: txs.clone(),
            batch_id: 0, // Will be determined after inserting to the database
        };

        if self.mempool_state.chunks_for_batch(&batch) > self.max_block_size_chunks {
            return Err(TxAddError::BatchTooBig);
        }

        let mut number_of_withdrawals = 0;
        for tx in txs {
            if tx.tx.is_withdraw() {
                number_of_withdrawals += 1;
            }
        }
        if number_of_withdrawals > self.max_number_of_withdrawals_per_block {
            return Err(TxAddError::BatchWithdrawalsOverload);
        }

        let mut transaction = storage.start_transaction().await.map_err(|err| {
            log::warn!("Mempool storage access error: {}", err);
            TxAddError::DbError
        })?;
        let batch_id = transaction
            .chain()
            .mempool_schema()
            .insert_batch(&batch.txs)
            .await
            .map_err(|err| {
                log::warn!("Mempool storage access error: {}", err);
                TxAddError::DbError
            })?;
        transaction.commit().await.map_err(|err| {
            log::warn!("Mempool storage access error: {}", err);
            TxAddError::DbError
        })?;

        batch.batch_id = batch_id;

        self.mempool_state.add_batch(batch)
    }

    async fn run(mut self) {
        while let Some(request) = self.requests.next().await {
            match request {
                MempoolRequest::NewTx(tx, resp) => {
                    let tx_add_result = self.add_tx(*tx).await;
                    resp.send(tx_add_result).unwrap_or_default();
                }
                MempoolRequest::NewTxsBatch(txs, resp) => {
                    let tx_add_result = self.add_batch(txs).await;
                    resp.send(tx_add_result).unwrap_or_default();
                }
                MempoolRequest::GetBlock(block) => {
                    // Generate proposed block.
                    let proposed_block =
                        self.propose_new_block(block.last_priority_op_number).await;

                    // Send the proposed block to the request initiator.
                    block
                        .response_sender
                        .send(proposed_block)
                        .expect("mempool proposed block response send failed");
                }
                MempoolRequest::UpdateNonces(updates) => {
                    for (id, update) in updates {
                        match update {
                            AccountUpdate::Create { address, nonce } => {
                                self.mempool_state.account_ids.insert(id, address);
                                self.mempool_state.account_nonces.insert(address, nonce);
                            }
                            AccountUpdate::Delete { address, .. } => {
                                self.mempool_state.account_ids.remove(&id);
                                self.mempool_state.account_nonces.remove(&address);
                            }
                            AccountUpdate::UpdateBalance { new_nonce, .. } => {
                                if let Some(address) = self.mempool_state.account_ids.get(&id) {
                                    if let Some(nonce) =
                                        self.mempool_state.account_nonces.get_mut(address)
                                    {
                                        *nonce = new_nonce;
                                    }
                                }
                            }
                            AccountUpdate::ChangePubKeyHash { new_nonce, .. } => {
                                if let Some(address) = self.mempool_state.account_ids.get(&id) {
                                    if let Some(nonce) =
                                        self.mempool_state.account_nonces.get_mut(address)
                                    {
                                        *nonce = new_nonce;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    async fn propose_new_block(&mut self, current_unprocessed_priority_op: u64) -> ProposedBlock {
        let (chunks_left, priority_ops) = self
            .select_priority_ops(current_unprocessed_priority_op)
            .await;
        let (_chunks_left, txs) = self.prepare_tx_for_block(chunks_left);

        log::trace!("Proposed priority ops for block: {:#?}", priority_ops);
        log::trace!("Proposed txs for block: {:#?}", txs);
        ProposedBlock { priority_ops, txs }
    }

    /// Returns: chunks left from max amount of chunks, ops selected
    async fn select_priority_ops(
        &self,
        current_unprocessed_priority_op: u64,
    ) -> (usize, Vec<PriorityOp>) {
        let eth_watch_resp = oneshot::channel();
        self.eth_watch_req
            .clone()
            .send(EthWatchRequest::GetPriorityQueueOps {
                op_start_id: current_unprocessed_priority_op,
                max_chunks: self.max_block_size_chunks,
                resp: eth_watch_resp.0,
            })
            .await
            .expect("ETH watch req receiver dropped");

        let priority_ops = eth_watch_resp.1.await.expect("Err response from eth watch");

        (
            self.max_block_size_chunks
                - priority_ops
                    .iter()
                    .map(|op| op.data.chunks())
                    .sum::<usize>(),
            priority_ops,
        )
    }

    fn prepare_tx_for_block(&mut self, mut chunks_left: usize) -> (usize, Vec<SignedTxVariant>) {
        let mut txs_for_commit = Vec::new();

        while let Some(tx) = self.mempool_state.ready_txs.pop_front() {
            let chunks_for_tx = self.mempool_state.required_chunks(&tx);
            if chunks_left >= chunks_for_tx {
                txs_for_commit.push(tx);
                chunks_left -= chunks_for_tx;
            } else {
                // Push the taken tx back, it does not fit.
                self.mempool_state.ready_txs.push_front(tx);
                break;
            }
        }

        (chunks_left, txs_for_commit)
    }
}

#[must_use]
pub fn run_mempool_task(
    db_pool: ConnectionPool,
    requests: mpsc::Receiver<MempoolRequest>,
    eth_watch_req: mpsc::Sender<EthWatchRequest>,
    config: &ConfigurationOptions,
) -> JoinHandle<()> {
    let config = config.clone();
    tokio::spawn(async move {
        let mempool_state = MempoolState::restore_from_db(&db_pool).await;

        let mempool = Mempool {
            db_pool,
            mempool_state,
            requests,
            eth_watch_req,
            max_block_size_chunks: *config
                .available_block_chunk_sizes
                .iter()
                .max()
                .expect("failed to find max block chunks size"),
            max_number_of_withdrawals_per_block: config.max_number_of_withdrawals_per_block,
        };

        mempool.run().await
    })
}
