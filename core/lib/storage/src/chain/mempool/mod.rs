// Built-in deps
use std::collections::VecDeque;
// External imports
use itertools::Itertools;
// Workspace imports
use zksync_types::{mempool::SignedTxVariant, tx::TxHash, SignedZkSyncTx};
// Local imports
use self::records::MempoolTx;
use crate::{QueryResult, StorageProcessor};

pub mod records;

/// Schema for persisting transactions awaiting for the execution.
///
/// This schema holds the transactions that are received by the `mempool` module, but not yet have
/// been included into some block. It is required to store these transactions in the database, so
/// in case of the unexpected server reboot sent transactions won't disappear, and will be executed
/// as if the server haven't been relaunched.
#[derive(Debug)]
pub struct MempoolSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> MempoolSchema<'a, 'c> {
    /// Loads all the transactions stored in the mempool schema.
    pub async fn load_txs(&mut self) -> QueryResult<VecDeque<SignedTxVariant>> {
        // Load the transactions from mempool along with corresponding batch IDs.
        let txs: Vec<MempoolTx> = sqlx::query_as!(
            MempoolTx,
            "SELECT * FROM mempool_txs
            ORDER BY created_at",
        )
        .fetch_all(self.0.conn())
        .await?;

        // Handles special case: batch_id == 0 <==> transaction is not a part of some batch
        fn batch_id_optional(batch_id: i64) -> Option<i64> {
            match batch_id {
                0 => None,
                _ => Some(batch_id),
            }
        };

        let mut prev_batch_id = txs
            .first()
            .map(|tx| batch_id_optional(tx.batch_id))
            .flatten();

        let grouped_txs = txs.into_iter().group_by(|tx| {
            prev_batch_id = batch_id_optional(tx.batch_id);

            prev_batch_id
        });

        let mut txs = Vec::new();

        for (batch_id, group) in grouped_txs.into_iter() {
            let deserialized_txs: Vec<SignedZkSyncTx> = group
                .map(|tx_object| -> QueryResult<SignedZkSyncTx> {
                    let tx = serde_json::from_value(tx_object.tx)?;
                    let sign_data = match tx_object.eth_sign_data {
                        None => None,
                        Some(sign_data_value) => serde_json::from_value(sign_data_value)?,
                    };

                    Ok(SignedZkSyncTx {
                        tx,
                        eth_sign_data: sign_data,
                    })
                })
                .collect::<Result<Vec<SignedZkSyncTx>, anyhow::Error>>()?;

            match batch_id {
                Some(batch_id) => {
                    // Group of batched transactions.
                    let variant = SignedTxVariant::batch(deserialized_txs, batch_id);
                    txs.push(variant);
                }
                None => {
                    // Group of non-batched transactions.
                    let mut variants = deserialized_txs
                        .into_iter()
                        .map(SignedTxVariant::from)
                        .collect();
                    txs.append(&mut variants);
                }
            }
        }

        // Now transactions should be sorted by the nonce (transaction natural order)
        // According to our convention in batch `fee transaction` would be the last one, so we would use nonce from it as a key for sort
        txs.sort_by_key(|tx| match tx {
            SignedTxVariant::Tx(tx) => tx.tx.nonce(),
            SignedTxVariant::Batch(batch) => batch
                .txs
                .last()
                .expect("batch must contain at least one transaction")
                .tx
                .nonce(),
        });

        Ok(txs.into())
    }

    /// Adds a new transactions batch to the mempool schema.
    /// Returns id of the inserted batch
    pub async fn insert_batch(&mut self, txs: &[SignedZkSyncTx]) -> QueryResult<i64> {
        if txs.is_empty() {
            anyhow::bail!("Cannot insert an empty batch");
        }

        // The first transaction of the batch would be inserted manually
        // batch_id of the inserted transaction would be the id of this batch
        // Will be unique cause batch_id is bigserial
        // Special case: batch_id == 0 <==> transaction is not a part of some batch (uses in `insert_tx` function)
        let batch_id = {
            let first_tx_data = txs[0].clone();
            let tx_hash = hex::encode(first_tx_data.hash().as_ref());
            let tx = serde_json::to_value(&first_tx_data.tx)
                .expect("Unserializable TX provided to the database");
            let eth_sign_data = first_tx_data
                .eth_sign_data
                .as_ref()
                .map(|sd| serde_json::to_value(sd).expect("failed to encode EthSignData"));

            sqlx::query!(
                "INSERT INTO mempool_txs (tx_hash, tx, created_at, eth_sign_data)
                VALUES ($1, $2, $3, $4)",
                tx_hash,
                tx,
                chrono::Utc::now(),
                eth_sign_data,
            )
            .execute(self.0.conn())
            .await?;

            sqlx::query_as!(
                MempoolTx,
                "SELECT * FROM mempool_txs
                ORDER BY batch_id DESC
                LIMIT 1",
            )
            .fetch_optional(self.0.conn())
            .await?
            .ok_or_else(|| anyhow::format_err!("Can't get maximal batch_id from mempool_txs"))?
            .batch_id
        };

        // Processing of all batch transactions, except the first
        for tx_data in txs[1..].iter() {
            let tx_hash = hex::encode(tx_data.hash().as_ref());
            let tx = serde_json::to_value(&tx_data.tx)
                .expect("Unserializable TX provided to the database");
            let eth_sign_data = tx_data
                .eth_sign_data
                .as_ref()
                .map(|sd| serde_json::to_value(sd).expect("failed to encode EthSignData"));

            sqlx::query!(
                "INSERT INTO mempool_txs (tx_hash, tx, created_at, eth_sign_data, batch_id)
                VALUES ($1, $2, $3, $4, $5)",
                tx_hash,
                tx,
                chrono::Utc::now(),
                eth_sign_data,
                batch_id,
            )
            .execute(self.0.conn())
            .await?;
        }

        Ok(batch_id)
    }

    /// Adds a new transaction to the mempool schema.
    pub async fn insert_tx(&mut self, tx_data: &SignedZkSyncTx) -> QueryResult<()> {
        let tx_hash = hex::encode(tx_data.tx.hash().as_ref());
        let tx = serde_json::to_value(&tx_data.tx)?;
        let batch_id = 0; // Special case: batch_id == 0 <==> transaction is not a part of some batch

        let eth_sign_data = tx_data
            .eth_sign_data
            .as_ref()
            .map(|sd| serde_json::to_value(sd).expect("failed to encode EthSignData"));

        sqlx::query!(
            "INSERT INTO mempool_txs (tx_hash, tx, created_at, eth_sign_data, batch_id)
            VALUES ($1, $2, $3, $4, $5)",
            tx_hash,
            tx,
            chrono::Utc::now(),
            eth_sign_data,
            batch_id,
        )
        .execute(self.0.conn())
        .await?;

        Ok(())
    }

    pub async fn remove_tx(&mut self, tx: &[u8]) -> QueryResult<()> {
        let tx_hash = hex::encode(tx);

        sqlx::query!(
            "DELETE FROM mempool_txs
            WHERE tx_hash = $1",
            &tx_hash
        )
        .execute(self.0.conn())
        .await?;

        Ok(())
    }

    pub async fn remove_txs(&mut self, txs: &[TxHash]) -> QueryResult<()> {
        let tx_hashes: Vec<_> = txs.iter().map(hex::encode).collect();

        sqlx::query!(
            "DELETE FROM mempool_txs
            WHERE tx_hash = ANY($1)",
            &tx_hashes
        )
        .execute(self.0.conn())
        .await?;

        Ok(())
    }

    /// Removes transactions that are already committed.
    /// Though it's unlikely that mempool schema will ever contain a committed
    /// transaction, it's better to ensure that we won't process the same transaction
    /// again. One possible scenario for having already-processed txs in the database
    /// is a failure of `remove_txs` method, which won't cause a panic on server, but will
    /// left txs in the database.
    ///
    /// This method is expected to be initially invoked on the server start, and then
    /// invoked periodically with a big interval (to prevent possible database bloating).
    pub async fn collect_garbage(&mut self) -> QueryResult<()> {
        let all_txs: Vec<_> = self.load_txs().await?.into_iter().collect();
        let mut tx_hashes_to_remove = Vec::new();

        for tx in all_txs {
            let should_remove = match &tx {
                SignedTxVariant::Tx(tx) => {
                    let tx_hash = tx.hash();
                    self.0
                        .chain()
                        .operations_ext_schema()
                        .get_tx_by_hash(tx_hash.as_ref())
                        .await
                        .expect("DB issue while restoring the mempool state")
                        .is_some()
                }
                SignedTxVariant::Batch(batch) => {
                    // We assume that for batch one executed transaction <=> all the transactions are executed.
                    let tx_hash = batch.txs[0].hash();
                    self.0
                        .chain()
                        .operations_ext_schema()
                        .get_tx_by_hash(tx_hash.as_ref())
                        .await
                        .expect("DB issue while restoring the mempool state")
                        .is_some()
                }
            };

            if should_remove {
                tx_hashes_to_remove.extend(tx.hashes())
            }
        }

        self.remove_txs(&tx_hashes_to_remove).await?;

        Ok(())
    }
}
