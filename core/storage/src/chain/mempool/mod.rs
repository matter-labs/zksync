// Built-in deps
use std::collections::VecDeque;
// External imports
// Workspace imports
use models::node::{tx::TxHash, SignedFranklinTx};
// Local imports
use self::records::MempoolTx;
use crate::{QueryResult, StorageProcessor};

pub mod records;

/// Schema for TODO
#[derive(Debug)]
pub struct MempoolSchema<'a>(pub &'a mut StorageProcessor);

impl<'a> MempoolSchema<'a> {
    /// Loads all the transactions stored in the mempool schema.
    pub async fn load_txs(&mut self) -> QueryResult<VecDeque<SignedFranklinTx>> {
        let txs: Vec<MempoolTx> = sqlx::query_as!(
            MempoolTx,
            "SELECT * FROM mempool_txs
            ORDER BY created_at",
        )
        .fetch_all(self.0.conn())
        .await?;

        let mut txs = txs
            .into_iter()
            .map(|tx_object| -> QueryResult<SignedFranklinTx> {
                let tx = serde_json::from_value(tx_object.tx)?;
                let sign_data = match tx_object.eth_sign_data {
                    None => None,
                    Some(sign_data_value) => serde_json::from_value(sign_data_value)?,
                };

                Ok(SignedFranklinTx {
                    tx,
                    eth_sign_data: sign_data,
                })
            })
            .collect::<Result<Vec<SignedFranklinTx>, _>>()?;
        txs.sort_by_key(|signed_tx| signed_tx.tx.nonce());
        Ok(txs.into())
    }

    /// Adds a new transaction to the mempool schema.
    pub async fn insert_tx(&mut self, tx_data: &SignedFranklinTx) -> QueryResult<()> {
        let tx_hash = hex::encode(tx_data.tx.hash().as_ref());
        let tx = serde_json::to_value(&tx_data.tx)?;

        let eth_sign_data = tx_data
            .eth_sign_data
            .as_ref()
            .map(|sd| serde_json::to_value(sd).expect("failed to encode EthSignData"));

        sqlx::query!(
            "INSERT INTO mempool_txs (tx_hash, tx, created_at, eth_sign_data)
            VALUES ($1, $2, $3, $4)",
            tx_hash,
            tx,
            chrono::Utc::now(),
            eth_sign_data
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

    async fn remove_txs(&mut self, txs: &[TxHash]) -> QueryResult<()> {
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
            let tx_hash = tx.hash();
            let should_remove = self
                .0
                .chain()
                .operations_ext_schema()
                .get_tx_by_hash(tx_hash.as_ref())
                .await
                .expect("DB issue while restoring the mempool state")
                .is_some();

            if should_remove {
                tx_hashes_to_remove.push(tx.hash())
            }
        }

        self.remove_txs(&tx_hashes_to_remove).await?;

        Ok(())
    }
}
