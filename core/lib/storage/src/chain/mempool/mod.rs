// Built-in deps
use std::{collections::VecDeque, convert::TryFrom, str::FromStr, time::Instant};
// External imports
use itertools::Itertools;
// Workspace imports
use zksync_api_types::v02::pagination::PaginationDirection;
use zksync_api_types::v02::transaction::{
    ApiTxBatch, BatchStatus, TxHashSerializeWrapper, TxInBlockStatus,
};
use zksync_types::{
    block::IncompleteBlock,
    mempool::SignedTxVariant,
    tx::{TxEthSignature, TxHash},
    AccountId, Address, BlockNumber, ExecutedOperations, ExecutedPriorityOp, ExecutedTx,
    PriorityOp, SerialId, SignedZkSyncTx, ZkSyncPriorityOp, H256,
};
// Local imports
use self::records::{MempoolPriorityOp, MempoolTx, QueuedBatchTx, RevertedBlock};
use crate::{QueryResult, StorageProcessor};

use crate::chain::operations::records::{
    StoredExecutedPriorityOperation, StoredExecutedTransaction,
};

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
    /// Loads all transactions stored in the mempool schema.
    /// We want to exclude txs that have already been processed in memory,
    /// due to asynchronous execution,
    /// these txs may be executed in memory and not yet saved to the database
    pub async fn load_txs(
        &mut self,
        executed_txs: &[TxHash],
    ) -> QueryResult<VecDeque<SignedTxVariant>> {
        let start = Instant::now();
        // Load the transactions from mempool along with corresponding batch IDs.
        let excluded_txs: Vec<String> = executed_txs
            .iter()
            .map(|tx| tx.to_string_without_prefix())
            .collect();
        let txs: Vec<MempoolTx> = sqlx::query_as!(
            MempoolTx,
            "SELECT * FROM mempool_txs WHERE reverted = false AND tx_hash NOT IN (
                SELECT u.hashes FROM UNNEST ($1::text[]) as u(hashes)
            )
            ORDER BY id
            LIMIT 400
            ",
            &excluded_txs
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

        for (batch_id, group) in grouped_txs.into_iter() {
            if let Some(batch_id) = batch_id {
                let deserialized_txs = group
                    .map(SignedZkSyncTx::try_from)
                    .collect::<Result<Vec<SignedZkSyncTx>, serde_json::Error>>()?;
                let variant = SignedTxVariant::batch(deserialized_txs, batch_id, vec![]);

                txs.push(variant);
            } else {
                for mempool_tx in group {
                    let signed_tx = SignedZkSyncTx::try_from(mempool_tx)?;
                    let variant = SignedTxVariant::Tx(signed_tx);
                    txs.push(variant);
                }
            }
        }

        // Load signatures for batches.
        for tx in txs.iter_mut() {
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
        Ok(txs.into())
    }

    pub async fn remove_reverted_block(&mut self, block_number: BlockNumber) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;
        sqlx::query!(
            "DELETE FROM reverted_block WHERE number = $1",
            *block_number as i64
        )
        .execute(transaction.conn())
        .await?;
        sqlx::query!(
            "DELETE FROM mempool_reverted_txs_meta WHERE block_number = $1",
            *block_number as i64
        )
        .execute(transaction.conn())
        .await?;
        transaction.commit().await?;
        metrics::histogram!("sql.chain.mempool.remove_reverted_block", start.elapsed());
        Ok(())
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
                first_tx_data.created_at,
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

        for (tx_data, tx_hash) in txs[1..].iter().zip(tx_hashes[1..].iter()) {
            let tx_hash = hex::encode(tx_hash.as_ref());
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
                tx_data.created_at,
                eth_sign_data,
                batch_id
            )
            .execute(transaction.conn())
            .await?;
        }

        // If there are signatures for the whole batch, store them too.
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
            tx_data.created_at,
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
    pub async fn get_tx(&mut self, tx_hash: &[u8]) -> QueryResult<Option<SignedZkSyncTx>> {
        let start = Instant::now();

        let mempool_tx = self.get_mempool_tx(tx_hash).await?;

        metrics::histogram!("sql.chain", start.elapsed(), "mempool" => "get_tx");
        mempool_tx
            .map(SignedZkSyncTx::try_from)
            .transpose()
            .map_err(anyhow::Error::from)
    }

    /// Returns mempool transaction as it is stored in the database.
    async fn get_mempool_tx(&mut self, tx_hash: &[u8]) -> QueryResult<Option<MempoolTx>> {
        let start = Instant::now();

        let tx_hash = hex::encode(tx_hash);

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
        let all_txs = self.load_txs(&[]).await?;
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

        let priority_ops = self.get_confirmed_priority_ops().await?;
        let mut priority_ops_to_remove = Vec::new();
        for op in priority_ops {
            let should_remove = self
                .0
                .chain()
                .operations_schema()
                .get_executed_priority_operation(op.serial_id as u32)
                .await?
                .is_some();
            if should_remove {
                priority_ops_to_remove.push(op.serial_id);
            }
        }

        self.remove_priority_ops_from_mempool(&priority_ops_to_remove)
            .await?;

        metrics::histogram!("sql.chain.mempool.collect_garbage", start.elapsed());
        Ok(())
    }

    pub async fn insert_priority_ops(
        &mut self,
        ops: &[PriorityOp],
        confirmed: bool,
    ) -> QueryResult<()> {
        let start = Instant::now();
        // Multi insert in this specific scenario is less convenient,
        // because we have to `DO UPDATE`.
        // We `DO UPDATE` for two cases, first of all we must confirm the priority operations
        // and the next scenario we work with network splits,
        // and until we execute priority op the data under serial_id may be different

        let mut transaction = self.0.start_transaction().await?;
        for op in ops {
            let serial_id = op.serial_id as i64;
            let tx_hash = hex::encode(op.tx_hash().as_ref());
            let data = serde_json::to_value(op.data.clone()).expect("Should be encoded");
            let deadline_block = op.deadline_block as i64;
            let eth_hash = op.eth_hash.as_bytes().to_vec();
            let eth_block = op.eth_block as i64;
            let eth_block_index = op.eth_block_index.map(|v| v as i32).unwrap_or_default();
            let op_type = op.data.variance_name();
            let (l1_address, l2_address) = match &op.data {
                ZkSyncPriorityOp::Deposit(dep) => {
                    (dep.from.as_bytes().to_vec(), dep.to.as_bytes().to_vec())
                }
                ZkSyncPriorityOp::FullExit(fe) => (
                    fe.eth_address.as_bytes().to_vec(),
                    fe.eth_address.as_bytes().to_vec(),
                ),
            };

            sqlx::query!(
                "INSERT INTO mempool_priority_operations (
                    serial_id, data, deadline_block, eth_hash, tx_hash,
                    eth_block, eth_block_index, l1_address, 
                    l2_address, type, created_at, confirmed
                 )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, now(), $11)
                ON CONFLICT (serial_id) DO UPDATE SET
                data=$2, deadline_block=$3, eth_hash=$4, tx_hash=$5,
                eth_block=$6, eth_block_index=$7, l1_address=$8,
                l2_address=$9, type=$10, confirmed=$11
                ",
                serial_id,
                data,
                deadline_block,
                eth_hash,
                tx_hash,
                eth_block,
                eth_block_index,
                l1_address,
                l2_address,
                op_type,
                confirmed
            )
            .execute(transaction.conn())
            .await?;
        }
        transaction.commit().await?;
        metrics::histogram!("sql.chain", start.elapsed(), "schema" => "mempool", "method" => "insert_priority_ops");
        Ok(())
    }

    pub async fn get_confirmed_priority_ops(&mut self) -> QueryResult<VecDeque<PriorityOp>> {
        let ops = sqlx::query_as!(
            MempoolPriorityOp,
            "SELECT serial_id,data,deadline_block,eth_hash,tx_hash,eth_block,eth_block_index,created_at FROM mempool_priority_operations WHERE confirmed AND reverted = false ORDER BY serial_id"
        )
        .fetch_all(self.0.conn())
        .await?;
        Ok(ops.into_iter().map(|op| op.into()).collect())
    }

    pub async fn remove_priority_op_from_mempool(&mut self, id: i64) -> QueryResult<()> {
        sqlx::query!(
            "DELETE FROM mempool_priority_operations WHERE serial_id=$1",
            id
        )
        .execute(self.0.conn())
        .await?;
        Ok(())
    }

    pub async fn get_max_serial_id_pending_deposits(
        &mut self,
        address: Address,
    ) -> QueryResult<Option<SerialId>> {
        let serial_id = sqlx::query!(
            "SELECT max(serial_id) FROM mempool_priority_operations WHERE l2_address = $1",
            address.as_bytes().to_vec()
        )
        .fetch_one(self.0.conn())
        .await?
        .max;
        Ok(serial_id.map(|v| v as u64))
    }

    pub async fn get_pending_deposits_for(
        &mut self,
        address: Address,
        start_serial_id: SerialId,
        limit: u32,
        direction: PaginationDirection,
    ) -> QueryResult<Vec<PriorityOp>> {
        let query = "SELECT serial_id,data,deadline_block,eth_hash,tx_hash,eth_block,eth_block_index,created_at FROM mempool_priority_operations WHERE l2_address = $1";
        let query = match direction {
            PaginationDirection::Newer => {
                format!("{} AND serial_id >= $2 ORDER BY serial_id LIMIT $3", query)
            }
            PaginationDirection::Older => {
                format!(
                    "{} AND serial_id <= $2 ORDER BY serial_id DESC LIMIT $3",
                    query
                )
            }
        };
        let ops: Vec<MempoolPriorityOp> = sqlx::query_as(query.as_str())
            .bind(address.as_bytes().to_vec())
            .bind(start_serial_id as i64)
            .bind(limit as i64)
            .fetch_all(self.0.conn())
            .await?;
        Ok(ops.into_iter().map(|op| op.into()).collect())
    }

    pub async fn get_pending_operation_by_hash(
        &mut self,
        tx_hash: H256,
    ) -> QueryResult<Option<PriorityOp>> {
        let op = sqlx::query_as!(
            MempoolPriorityOp,
            r#"
                SELECT serial_id,data,deadline_block,eth_hash,
                       tx_hash,eth_block,eth_block_index,created_at 
                FROM mempool_priority_operations 
                WHERE eth_hash = $1
            "#,
            tx_hash.as_bytes().to_vec()
        )
        .fetch_optional(self.0.conn())
        .await?
        .map(|op| op.into());
        Ok(op)
    }
    pub async fn get_pending_deposits(&mut self, address: Address) -> QueryResult<Vec<PriorityOp>> {
        let ops = sqlx::query_as!(
            MempoolPriorityOp,
            r#"
            SELECT serial_id,data,deadline_block,eth_hash,
                   tx_hash,eth_block,eth_block_index,created_at 
            FROM mempool_priority_operations 
            WHERE type = 'Deposit' AND l2_address = $1  
            ORDER BY serial_id"#,
            address.as_bytes().to_vec()
        )
        .fetch_all(self.0.conn())
        .await?;
        Ok(ops.into_iter().map(|op| op.into()).collect())
    }

    pub async fn remove_priority_ops_from_mempool(&mut self, ids: &[u64]) -> QueryResult<()> {
        let ids: Vec<_> = ids.iter().map(|v| *v as i64).collect();
        sqlx::query!(
            "DELETE FROM mempool_priority_operations WHERE serial_id=ANY($1)",
            &ids
        )
        .execute(self.0.conn())
        .await?;
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

    pub async fn get_reverted_blocks(
        &mut self,
        available_block_sizes: &[usize],
        fee_account_id: AccountId,
    ) -> QueryResult<VecDeque<IncompleteBlock>> {
        let mut transaction = self.0.start_transaction().await?;
        let blocks = sqlx::query_as!(
            RevertedBlock,
            "SELECT * FROM reverted_block ORDER BY number"
        )
        .fetch_all(transaction.conn())
        .await?;
        let mut incomplete_blocks = VecDeque::new();
        for block in blocks {
            let mut executed_operations = Vec::new();
            let executed_ops = sqlx::query_as!(
                StoredExecutedTransaction,
               r#"
                SELECT 
                -- We don't use sequence number here, so we can just skip it.
                Null::bigint as sequence_number,
                mempool_reverted_txs_meta.block_number, 
                mempool_reverted_txs_meta.block_index, 
                mempool_txs.tx, 
                mempool_reverted_txs_meta.nonce as "nonce!", 
                mempool_reverted_txs_meta.operation, 
                mempool_reverted_txs_meta.tx_hash_bytes as tx_hash,
                mempool_reverted_txs_meta.from_account,
                mempool_reverted_txs_meta.to_account,
                mempool_reverted_txs_meta.success,
                mempool_reverted_txs_meta.fail_reason,
                mempool_reverted_txs_meta.primary_account_address,
                mempool_txs.created_at,
                mempool_txs.eth_sign_data,
                mempool_txs.batch_id as "batch_id?"
                FROM mempool_txs INNER JOIN mempool_reverted_txs_meta 
                ON mempool_txs.tx_hash = mempool_reverted_txs_meta.tx_hash 
                WHERE mempool_reverted_txs_meta.block_number=$1 AND mempool_reverted_txs_meta.tx_type='L2'"#, 
                block.number
            ).fetch_all(transaction.conn()).await?;

            let executed_ops = executed_ops
                .into_iter()
                .map(|stored_exec| stored_exec.into_executed_tx())
                .map(|tx| ExecutedOperations::Tx(Box::new(tx)));
            executed_operations.extend(executed_ops);
            let executed_priority_ops = sqlx::query_as!(
                StoredExecutedPriorityOperation,
                r#"SELECT 
                -- We don't use sequence number here, so we can just skip it.
                Null::bigint as sequence_number,
                mempool_reverted_txs_meta.block_number, 
                mempool_reverted_txs_meta.block_index as "block_index!", 
                mempool_reverted_txs_meta.operation, 
                mempool_reverted_txs_meta.from_account,
                mempool_reverted_txs_meta.to_account as "to_account!",
                mempool_priority_operations.serial_id as priority_op_serialid,
                mempool_priority_operations.deadline_block,
                mempool_priority_operations.eth_hash,
                mempool_priority_operations.eth_block,
                mempool_priority_operations.created_at,
                cast(mempool_priority_operations.eth_block_index as bigint) as "eth_block_index?",
                mempool_reverted_txs_meta.tx_hash_bytes as tx_hash
                 FROM mempool_priority_operations INNER JOIN mempool_reverted_txs_meta 
                ON mempool_priority_operations.tx_hash = mempool_reverted_txs_meta.tx_hash 
                WHERE mempool_reverted_txs_meta.block_number=$1 AND mempool_reverted_txs_meta.tx_type='L1'"#, 
                block.number
            ).fetch_all(transaction.conn()).await?;
            let executed_priority_ops = executed_priority_ops
                .into_iter()
                .map(|op| ExecutedOperations::PriorityOp(Box::new(op.into_executed())));

            executed_operations.extend(executed_priority_ops);
            executed_operations.sort_by_key(|exec_op| {
                match exec_op {
                    ExecutedOperations::Tx(tx) => {
                        if let Some(idx) = tx.block_index {
                            idx
                        } else {
                            // failed operations are at the end.
                            u32::MAX
                        }
                    }
                    ExecutedOperations::PriorityOp(op) => op.block_index,
                }
            });

            incomplete_blocks.push_back(IncompleteBlock::new_from_available_block_sizes(
                BlockNumber(block.number as u32),
                fee_account_id,
                executed_operations,
                (
                    block.unprocessed_priority_op_before as u64,
                    block.unprocessed_priority_op_after as u64,
                ),
                available_block_sizes,
                Default::default(),
                Default::default(),
                block.timestamp as u64,
            ));
        }
        transaction.commit().await?;
        Ok(incomplete_blocks)
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
        let mut reverted_operations = Vec::new();
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

            let block_for_revert = transaction
                .chain()
                .block_schema()
                .get_block(block_number)
                .await?;
            let (number, processed_priority_ops_before, processed_priority_ops_after, timestamp) =
                if let Some(block) = block_for_revert {
                    (
                        *block.block_number as i64,
                        block.processed_priority_ops.0 as i64,
                        block.processed_priority_ops.1 as i64,
                        block.timestamp as i64,
                    )
                } else {
                    let (block_for_revert, _) = transaction
                        .chain()
                        .block_schema()
                        .get_data_to_complete_block(block_number)
                        .await?;
                    let data = if let Some(block) = block_for_revert {
                        (
                            *block.block_number as i64,
                            block.processed_priority_ops.0 as i64,
                            block.processed_priority_ops.1 as i64,
                            block.timestamp as i64,
                        )
                    } else {
                        let block = transaction
                            .chain()
                            .block_schema()
                            .load_pending_block()
                            .await?
                            .expect("Block does not exist");

                        let unprocessed_priority_op_after = block.unprocessed_priority_op_before
                            + block_transactions
                                .iter()
                                .filter(|ex| ex.is_priority())
                                .count() as u64;
                        (
                            *block.number as i64,
                            block.unprocessed_priority_op_before as i64,
                            unprocessed_priority_op_after as i64,
                            block.timestamp as i64,
                        )
                    };
                    data
                };

            sqlx::query!(
                r#"
                INSERT INTO reverted_block (
                    number, unprocessed_priority_op_before, 
                    unprocessed_priority_op_after, timestamp
                ) VALUES ( $1, $2, $3, $4 )"#,
                number,
                processed_priority_ops_before,
                processed_priority_ops_after,
                timestamp,
            )
            .execute(transaction.conn())
            .await?;
            vlog::info!("Reverting transactions from the block {}", block_number);

            for executed_tx in block_transactions {
                if !executed_tx.is_successful() {
                    continue;
                }
                match executed_tx {
                    ExecutedOperations::Tx(tx) => {
                        reverted_txs.push((tx, block_number, next_priority_op_serial_id));
                    }
                    ExecutedOperations::PriorityOp(priority_op) => {
                        assert_eq!(
                            priority_op.priority_op.serial_id,
                            next_priority_op_serial_id
                        );
                        next_priority_op_serial_id += 1;
                        reverted_operations.push((priority_op, block_number));
                    }
                }
            }

            block_number = block_number + 1;
        }

        for (reverted_tx, block_number, next_priority_op_serial_id) in reverted_txs {
            let ExecutedTx {
                signed_tx,
                success,
                created_at,
                batch_id,
                op,
                block_index,
                fail_reason,
            } = *reverted_tx;

            let block_index = block_index.map(|b| b as i32);
            let nonce = signed_tx.nonce();
            let from_account = signed_tx.from_account().as_bytes().to_vec();
            let to_account = signed_tx.to_account().map(|a| a.as_bytes().to_vec());
            let primary_account_address = signed_tx.account().as_bytes().to_vec();

            let SignedZkSyncTx {
                tx, eth_sign_data, ..
            } = signed_tx;

            let tx_hash_bytes = tx.hash().as_ref().to_vec();
            let tx_hash = hex::encode(&tx_hash_bytes);
            let tx_value =
                serde_json::to_value(tx).expect("Failed to serialize reverted transaction");
            let operation =
                serde_json::to_value(op).expect("Failed to serialize reverted transaction");
            let eth_sign_data = eth_sign_data.as_ref().map(|sign_data| {
                serde_json::to_value(sign_data).expect("Failed to serialize Ethereum sign data")
            });

            sqlx::query!(
                r#"INSERT INTO mempool_reverted_txs_meta (
                 tx_hash, operation, block_number, block_index, tx_hash_bytes, nonce, from_account, 
                 to_account, success, fail_reason, primary_account_address, tx_type
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'L2')"#,
                tx_hash,
                operation,
                *block_number as i64,
                block_index,
                tx_hash_bytes,
                *nonce as i64,
                from_account,
                to_account,
                success,
                fail_reason,
                primary_account_address,
            )
            .execute(transaction.conn())
            .await?;

            sqlx::query!(
                "INSERT INTO mempool_txs (tx_hash, tx, created_at, eth_sign_data, batch_id, next_priority_op_serial_id, reverted)
                VALUES ($1, $2, $3, $4, $5, $6, true)",
                tx_hash,
                tx_value,
                created_at,
                eth_sign_data,
                batch_id.unwrap_or(0i64),
                next_priority_op_serial_id as i64,
            )
            .execute(transaction.conn())
            .await?;

            sqlx::query!(
                "DELETE FROM tx_filters
                WHERE tx_hash = $1",
                &tx_hash_bytes
            )
            .execute(transaction.conn())
            .await?;
        }

        for (op, block_number) in reverted_operations {
            let ExecutedPriorityOp {
                priority_op,
                op,
                block_index,
                created_at,
            } = *op;

            let tx_hash_bytes = priority_op.tx_hash().as_ref().to_vec();
            let tx_hash = hex::encode(&tx_hash_bytes);
            let data = serde_json::to_value(&priority_op.data)
                .expect("Failed to serialize reverted transaction");
            let operation =
                serde_json::to_value(op).expect("Failed to serialize reverted transaction");

            let serial_id = priority_op.serial_id as i64;
            let l1_address = priority_op.data.from_account().as_bytes().to_vec();
            let l2_address = priority_op.data.to_account().as_bytes().to_vec();
            let op_type = priority_op.data.variance_name();
            let deadline_block = priority_op.deadline_block as i64;
            let eth_hash = priority_op.eth_hash.as_bytes().to_vec();
            let eth_block = priority_op.eth_block as i64;
            let eth_block_index = priority_op.eth_block_index.map(|a| a as i32);

            sqlx::query!(
                r#"INSERT INTO mempool_reverted_txs_meta (
                 tx_hash, operation, block_number, block_index, tx_hash_bytes, 
                 from_account, to_account, primary_account_address, 
                 success, tx_type
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, true, 'L1')"#,
                tx_hash,
                operation,
                *block_number as i64,
                block_index as i32,
                tx_hash_bytes,
                l1_address,
                l2_address,
                l2_address,
            )
            .execute(transaction.conn())
            .await?;

            sqlx::query!(
                "INSERT INTO mempool_priority_operations (
                    serial_id, data, l1_address, l2_address, 
                    type, deadline_block, eth_hash, tx_hash, eth_block, 
                    eth_block_index, created_at, confirmed, reverted
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, true, true)",
                serial_id,
                data,
                l1_address,
                l2_address,
                op_type,
                deadline_block,
                eth_hash,
                tx_hash,
                eth_block,
                eth_block_index,
                created_at
            )
            .execute(transaction.conn())
            .await?;

            sqlx::query!(
                "DELETE FROM tx_filters
                WHERE tx_hash = $1",
                &tx_hash_bytes
            )
            .execute(transaction.conn())
            .await?;
        }

        sqlx::query!(
            r"DELETE FROM executed_priority_operations 
            WHERE block_number > $1",
            *last_block_number as i64
        )
        .execute(transaction.conn())
        .await?;
        sqlx::query!(
            r"DELETE FROM executed_transactions
            WHERE block_number > $1",
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
