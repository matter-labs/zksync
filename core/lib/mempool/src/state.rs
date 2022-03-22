use std::collections::HashSet;
use std::time::Instant;

use zksync_storage::{ConnectionPool, StorageProcessor};
use zksync_types::{
    mempool::{SignedTxVariant, SignedTxsBatch},
    tx::{error::TxAddError, TxHash},
    Address, TransferOp, TransferToNewOp, ZkSyncTx,
};

use crate::MempoolTransactionsQueue;

#[derive(Debug, Clone)]
pub(crate) struct MempoolState {
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

    pub async fn chunks_for_batch(&self, batch: &SignedTxsBatch) -> Result<usize, TxAddError> {
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

    pub async fn required_chunks(&self, element: &SignedTxVariant) -> Result<usize, TxAddError> {
        match element {
            SignedTxVariant::Tx(tx) => self.chunks_for_tx(&tx.tx).await,
            SignedTxVariant::Batch(batch) => self.chunks_for_batch(batch).await,
        }
    }

    pub async fn collect_garbage(&self) {
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

    pub async fn get_transaction_queue(
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
