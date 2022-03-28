use futures::channel::{mpsc, oneshot};
use futures::StreamExt;

use zksync_storage::ConnectionPool;
use zksync_types::{
    mempool::SignedTxsBatch,
    tx::{error::TxAddError, TxEthSignature},
    PriorityOp, SignedZkSyncTx,
};

use crate::state::MempoolState;

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

pub(crate) struct MempoolTransactionsHandler {
    pub db_pool: ConnectionPool,
    pub mempool_state: MempoolState,
    pub requests: mpsc::Receiver<MempoolTransactionRequest>,
    pub max_block_size_chunks: usize,
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
        let batch = SignedTxsBatch {
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

    pub async fn run(mut self) {
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
