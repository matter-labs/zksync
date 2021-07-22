// Built-in deps
use std::{
    collections::VecDeque,
    convert::{TryFrom, TryInto},
    str::FromStr,
    time::Instant,
};
// External imports
use itertools::Itertools;
// Workspace imports
use zksync_api_types::v02::transaction::{
    ApiTxBatch, BatchStatus, TxHashSerializeWrapper, TxInBlockStatus,
};
use zksync_types::{
    mempool::{RevertedTxVariant, SignedTxVariant},
    tx::{TxEthSignature, TxHash},
    BlockNumber, ExecutedOperations, ExecutedTx, SignedZkSyncTx,
};
// Local imports
use self::records::{MempoolTx, QueuedBatchTx};
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
    pub async fn load_txs(
        &mut self,
    ) -> QueryResult<(VecDeque<SignedTxVariant>, VecDeque<RevertedTxVariant>)> {
        let start = Instant::now();
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
        }

        let mut prev_batch_id = txs.first().and_then(|tx| batch_id_optional(tx.batch_id));

        let grouped_txs = txs.into_iter().group_by(|tx| {
            prev_batch_id = batch_id_optional(tx.batch_id);

            prev_batch_id
        });

        let mut txs = Vec::new();
        let mut reverted_txs = Vec::new();

        for (batch_id, group) in grouped_txs.into_iter() {
            if let Some(batch_id) = batch_id {
                let mut group = group.peekable();
                let next_priority_op_serial_id = group.peek().unwrap().next_priority_op_serial_id;
                let deserialized_txs = group
                    .map(SignedZkSyncTx::try_from)
                    .collect::<Result<Vec<SignedZkSyncTx>, serde_json::Error>>()?;
                let variant = SignedTxVariant::batch(deserialized_txs, batch_id, vec![]);

                match next_priority_op_serial_id {
                    Some(serial_id) => {
                        reverted_txs.push(RevertedTxVariant::new(variant, serial_id.try_into()?));
                    }
                    None => txs.push(variant),
                }
            } else {
                for mempool_tx in group {
                    let next_priority_op_serial_id = mempool_tx.next_priority_op_serial_id;
                    let signed_tx = SignedZkSyncTx::try_from(mempool_tx)?;
                    let variant = SignedTxVariant::Tx(signed_tx);

                    match next_priority_op_serial_id {
                        Some(serial_id) => {
                            reverted_txs
                                .push(RevertedTxVariant::new(variant, serial_id.try_into()?));
                        }
                        None => txs.push(variant),
                    }
                }
            }
        }

        // Load signatures for batches.
        for tx in txs
            .iter_mut()
            .chain(reverted_txs.iter_mut().map(AsMut::as_mut))
        {
            if let SignedTxVariant::Batch(batch) = tx {
                let eth_signatures: Vec<TxEthSignature> = sqlx::query!(
                    "SELECT eth_signature FROM txs_batches_signatures
                    WHERE batch_id = $1",
                    batch.batch_id
                )
                .fetch_all(self.0.conn())
                .await?
                .into_iter()
                .map(|value| {
                    serde_json::from_value(value.eth_signature)
                        .expect("failed to decode TxEthSignature")
                })
                .collect();

                batch.eth_signatures = eth_signatures;
            }
        }

        metrics::histogram!("sql.chain.mempool.load_txs", start.elapsed());
        Ok((txs.into(), reverted_txs.into()))
    }

    /// Adds a new transactions batch to the mempool schema.
    /// Returns id of the inserted batch
    pub async fn insert_batch(
        &mut self,
        txs: &[SignedZkSyncTx],
        eth_signatures: Vec<TxEthSignature>,
    ) -> QueryResult<i64> {
        let start = Instant::now();
        if txs.is_empty() {
            anyhow::bail!("Cannot insert an empty batch");
        }

        let mut transaction = self.0.start_transaction().await?;
        let tx_hashes: Vec<TxHash> = txs.iter().map(|tx| tx.tx.hash()).collect();

        // The first transaction of the batch would be inserted manually
        // batch_id of the inserted transaction would be the id of this batch
        // Will be unique cause batch_id is bigserial
        // Special case: batch_id == 0 <==> transaction is not a part of some batch (uses in `insert_tx` function)
        let batch_id = {
            let first_tx_data = txs[0].clone();
            let tx_hash = hex::encode(tx_hashes[0].as_ref());
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
            .execute(transaction.conn())
            .await?;

            sqlx::query_as!(
                MempoolTx,
                "SELECT * FROM mempool_txs
                ORDER BY batch_id DESC
                LIMIT 1",
            )
            .fetch_optional(transaction.conn())
            .await?
            .ok_or_else(|| anyhow::format_err!("Can't get maximal batch_id from mempool_txs"))?
            .batch_id
        };

        // Processing of all batch transactions, except the first
        let mut tx_hashes_strs = Vec::with_capacity(txs.len());
        let mut tx_values = Vec::with_capacity(txs.len());
        let mut txs_sign_data = Vec::with_capacity(txs.len());

        for (tx_data, tx_hash) in txs[1..].iter().zip(tx_hashes[1..].iter()) {
            tx_hashes_strs.push(hex::encode(tx_hash.as_ref()));
            tx_values.push(
                serde_json::to_value(&tx_data.tx)
                    .expect("Unserializable TX provided to the database"),
            );
            txs_sign_data.push(
                tx_data
                    .eth_sign_data
                    .as_ref()
                    .map(|sd| serde_json::to_value(sd).expect("failed to encode EthSignData"))
                    .unwrap_or_default(),
            );
        }
        sqlx::query!(
            "INSERT INTO mempool_txs (tx_hash, tx, eth_sign_data, created_at, batch_id)
            SELECT u.tx_hash, u.tx, u.eth_sign_data, $4, $5
                FROM UNNEST ($1::text[], $2::jsonb[], $3::jsonb[])
                AS u(tx_hash, tx, eth_sign_data)",
            &tx_hashes_strs,
            &tx_values,
            &txs_sign_data,
            chrono::Utc::now(),
            batch_id
        )
        .execute(transaction.conn())
        .await?;

        // If there're signatures for the whole batch, store them too.
        for signature in eth_signatures {
            let signature = serde_json::to_value(signature)?;
            sqlx::query!(
                "INSERT INTO txs_batches_signatures VALUES($1, $2)",
                batch_id,
                signature
            )
            .execute(transaction.conn())
            .await?;
        }

        let batch_hash = TxHash::batch_hash(&tx_hashes);
        sqlx::query!(
            "INSERT INTO txs_batches_hashes VALUES($1, $2)",
            batch_id,
            batch_hash.as_ref()
        )
        .execute(transaction.conn())
        .await?;

        transaction.commit().await?;

        metrics::histogram!("sql.chain.mempool.insert_batch", start.elapsed());
        Ok(batch_id)
    }

    /// Adds a new transaction to the mempool schema.
    pub async fn insert_tx(&mut self, tx_data: &SignedZkSyncTx) -> QueryResult<()> {
        let start = Instant::now();
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

        metrics::histogram!("sql.chain.mempool.insert_tx", start.elapsed());
        Ok(())
    }

    pub async fn remove_tx(&mut self, tx: &[u8]) -> QueryResult<()> {
        let start = Instant::now();
        let tx_hash = hex::encode(tx);

        sqlx::query!(
            "DELETE FROM mempool_txs
            WHERE tx_hash = $1",
            &tx_hash
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.chain.mempool.remove_tx", start.elapsed());
        Ok(())
    }

    pub async fn remove_txs(&mut self, txs: &[TxHash]) -> QueryResult<()> {
        let start = Instant::now();
        let tx_hashes: Vec<_> = txs.iter().map(hex::encode).collect();

        sqlx::query!(
            "DELETE FROM mempool_txs
            WHERE tx_hash = ANY($1)",
            &tx_hashes
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.chain.mempool.remove_txs", start.elapsed());
        Ok(())
    }

    /// Checks if the memory pool contains transaction with the given hash.
    pub async fn contains_tx(&mut self, tx_hash: TxHash) -> QueryResult<bool> {
        let start = Instant::now();

        let tx_hash = hex::encode(tx_hash.as_ref());

        let row = sqlx::query!(
            "SELECT COUNT(*) from mempool_txs
            WHERE tx_hash = $1",
            &tx_hash
        )
        .fetch_one(self.0.conn())
        .await?
        .count;

        let contains = row.filter(|&counter| counter > 0).is_some();

        metrics::histogram!("sql.chain", start.elapsed(), "mempool" => "contains_tx");
        Ok(contains)
    }

    /// Returns zkSync transaction with the given hash.
    pub async fn get_tx(&mut self, tx_hash: TxHash) -> QueryResult<Option<SignedZkSyncTx>> {
        let start = Instant::now();

        let mempool_tx = self.get_mempool_tx(tx_hash).await?;

        metrics::histogram!("sql.chain", start.elapsed(), "mempool" => "get_tx");
        mempool_tx
            .map(SignedZkSyncTx::try_from)
            .transpose()
            .map_err(anyhow::Error::from)
    }

    /// Returns mempool transaction as it is stored in the database.
    pub async fn get_mempool_tx(&mut self, tx_hash: TxHash) -> QueryResult<Option<MempoolTx>> {
        let start = Instant::now();

        let tx_hash = hex::encode(tx_hash.as_ref());

        let mempool_tx = sqlx::query_as!(
            MempoolTx,
            "SELECT * from mempool_txs
            WHERE tx_hash = $1",
            &tx_hash
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!("sql.chain", start.elapsed(), "mempool" => "get_tx");
        Ok(mempool_tx)
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
        let start = Instant::now();
        let (queue, reverted_queue) = self.load_txs().await?;
        let all_txs: Vec<_> = queue
            .into_iter()
            .chain(
                reverted_queue
                    .into_iter()
                    .map(RevertedTxVariant::into_inner),
            )
            .collect();
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

        metrics::histogram!("sql.chain.mempool.collect_garbage", start.elapsed());
        Ok(())
    }

    /// Returns mempool size.
    pub async fn get_mempool_size(&mut self) -> QueryResult<u32> {
        let start = Instant::now();

        let size = sqlx::query!("SELECT COUNT(*) from mempool_txs")
            .fetch_one(self.0.conn())
            .await?
            .count;

        metrics::histogram!("sql.chain", start.elapsed(), "mempool" => "get_mempool_size");
        Ok(size.unwrap_or(0) as u32)
    }

    /// Get info about batch in mempool.
    pub async fn get_queued_batch_info(
        &mut self,
        batch_hash: TxHash,
    ) -> QueryResult<Option<ApiTxBatch>> {
        let start = Instant::now();

        let batch_data = sqlx::query_as!(
            QueuedBatchTx,
            r#"
                SELECT tx_hash, created_at
                FROM mempool_txs
                INNER JOIN txs_batches_hashes
                ON txs_batches_hashes.batch_id = mempool_txs.batch_id
                WHERE batch_hash = $1
                ORDER BY id ASC
            "#,
            batch_hash.as_ref()
        )
        .fetch_all(self.0.conn())
        .await?;
        let result = if !batch_data.is_empty() {
            let created_at = batch_data[0].created_at;
            let transaction_hashes: Vec<TxHashSerializeWrapper> = batch_data
                .iter()
                .map(|tx| {
                    TxHashSerializeWrapper(TxHash::from_str(&format!("0x{}", tx.tx_hash)).unwrap())
                })
                .collect();
            Some(ApiTxBatch {
                batch_hash,
                transaction_hashes,
                created_at,
                batch_status: BatchStatus {
                    updated_at: created_at,
                    last_state: TxInBlockStatus::Queued,
                },
            })
        } else {
            None
        };

        metrics::histogram!("sql.chain", start.elapsed(), "mempool" => "get_queued_batch_info");
        Ok(result)
    }

    // Returns executed txs back to mempool for blocks with number greater than `last_block`
    pub async fn return_executed_txs_to_mempool(
        &mut self,
        last_block_number: BlockNumber,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let last_block = transaction
            .chain()
            .block_schema()
            .get_block(last_block_number)
            .await?
            .ok_or_else(|| anyhow::Error::msg("Failed to load last block from the database"))?;

        let mut reverted_txs = Vec::new();
        let mut next_priority_op_serial_id = last_block.processed_priority_ops.1;
        let mut block_number = last_block_number + 1;

        loop {
            let block_transactions = transaction
                .chain()
                .block_schema()
                .get_block_executed_ops(block_number)
                .await?;
            if block_transactions.is_empty() {
                break;
            }
            vlog::info!("Reverting transactions from the block {}", block_number);

            for executed_tx in block_transactions {
                if !executed_tx.is_successful() {
                    continue;
                }

                match executed_tx {
                    ExecutedOperations::Tx(tx) => {
                        reverted_txs.push((tx, next_priority_op_serial_id));
                    }
                    ExecutedOperations::PriorityOp(priority_op) => {
                        assert_eq!(
                            priority_op.priority_op.serial_id,
                            next_priority_op_serial_id
                        );
                        next_priority_op_serial_id += 1;
                    }
                }
            }

            block_number = block_number + 1;
        }

        for (reverted_tx, next_priority_op_serial_id) in reverted_txs {
            let ExecutedTx {
                signed_tx,
                created_at,
                batch_id,
                ..
            } = *reverted_tx;
            let SignedZkSyncTx { tx, eth_sign_data } = signed_tx;

            let tx_hash = hex::encode(tx.hash().as_ref());
            let tx_value =
                serde_json::to_value(tx).expect("Failed to serialize reverted transaction");
            let eth_sign_data = eth_sign_data.as_ref().map(|sign_data| {
                serde_json::to_value(sign_data).expect("Failed to serialize Ethereum sign data")
            });

            sqlx::query!(
                "INSERT INTO mempool_txs (tx_hash, tx, created_at, eth_sign_data, batch_id, next_priority_op_serial_id)
                VALUES ($1, $2, $3, $4, $5, $6)",
                tx_hash,
                tx_value,
                created_at,
                eth_sign_data,
                batch_id.unwrap_or(0i64),
                next_priority_op_serial_id as i64,
            )
            .execute(transaction.conn())
            .await?;
        }

        sqlx::query!(
            r#"
            DELETE FROM executed_transactions
            WHERE block_number > $1
        "#,
            *last_block_number as i64
        )
        .execute(transaction.conn())
        .await?;
        transaction.commit().await?;

        metrics::histogram!(
            "sql.chain.mempool.return_executed_txs_to_mempool",
            start.elapsed()
        );
        Ok(())
    }
}
