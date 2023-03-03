// Built-in deps
use std::time::Instant;

// External imports
use chrono::{DateTime, Utc};

// Workspace imports
use zksync_api_types::{
    v02::{
        pagination::{AccountTxsRequest, PaginationDirection, PaginationQuery},
        transaction::{
            ApiTxBatch, BatchStatus, Receipt, Transaction, TxData, TxHashSerializeWrapper,
            TxInBlockStatus,
        },
    },
    Either,
};
use zksync_crypto::params;
use zksync_types::{
    aggregated_operations::AggregatedActionType, tx::TxHash, Address, BlockNumber, TokenId,
    ZkSyncOp, ZkSyncTx, H256,
};

// Local imports
use self::records::{
    AccountCreatedAt, InBlockBatchTx, PriorityOpReceiptResponse, StorageTxData, StorageTxReceipt,
    TransactionsHistoryItem, TxByHashResponse, TxReceiptResponse, Web3TxData, Web3TxReceipt,
};
use crate::chain::operations_ext::records::SequenceNumberRecord;
use crate::{
    chain::{
        block::records::TransactionItem,
        operations::{records::StoredExecutedPriorityOperation, OperationsSchema},
    },
    QueryResult, StorageProcessor,
};
use itertools::Itertools;

pub(crate) mod conversion;
pub mod records;

/// Direction to perform search of transactions to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDirection {
    /// Find transactions older than specified one.
    Older,
    /// Find transactions newer than specified one.
    Newer,
}

/// `OperationsExt` schema is a logical extension for an `Operations` schema,
/// which provides more getters for transactions.
/// While `Operations` getters are very basic, `OperationsExt` schema can transform
/// the data to be convenient for the caller.
#[derive(Debug)]
pub struct OperationsExtSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> OperationsExtSchema<'a, 'c> {
    pub async fn tx_receipt(&mut self, hash: &[u8]) -> QueryResult<Option<TxReceiptResponse>> {
        let start = Instant::now();
        let tx = OperationsSchema(self.0)
            .get_executed_operation(hash)
            .await?;

        let result = if let Some(tx) = tx {
            // Check whether transaction was verified.
            let verified = OperationsSchema(self.0)
                .get_stored_aggregated_operation(
                    BlockNumber(tx.block_number as u32),
                    AggregatedActionType::ExecuteBlocks,
                )
                .await
                .map(|operation| operation.confirmed)
                .unwrap_or_default();

            Ok(Some(TxReceiptResponse {
                tx_hash: hex::encode(hash),
                block_number: tx.block_number,
                success: tx.success,
                verified,
                fail_reason: tx.fail_reason,
                prover_run: None,
            }))
        } else {
            Ok(None)
        };

        metrics::histogram!("sql.chain.operations_ext.tx_receipt", start.elapsed());
        result
    }

    pub async fn tx_receipt_api_v02(&mut self, hash: &[u8]) -> QueryResult<Option<Receipt>> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;
        let hash_str = hex::encode(hash);
        let receipt: Option<StorageTxReceipt> = sqlx::query_as!(
            StorageTxReceipt,
            r#"
                WITH transaction AS (
                    SELECT
                        tx_hash,
                        block_number,
                        success,
                        fail_reason,
                        Null::bigint as eth_block,
                        Null::bigint as priority_op_serialid
                    FROM executed_transactions
                    WHERE tx_hash = $1
                ), priority_op AS (
                    SELECT
                        tx_hash,
                        block_number,
                        true as success,
                        Null as fail_reason,
                        eth_block,
                        priority_op_serialid
                    FROM executed_priority_operations
                    WHERE tx_hash = $1 OR eth_hash = $1
                ), mempool_tx AS (
                    SELECT
                        decode(tx_hash, 'hex'),
                        Null::bigint as block_number,
                        Null::boolean as success,
                        Null as fail_reason,
                        Null::bigint as eth_block,
                        Null::bigint as priority_op_serialid
                    FROM mempool_txs
                    WHERE tx_hash = $2
                ),
                everything AS (
                    SELECT * FROM transaction
                    UNION ALL
                    SELECT * FROM priority_op
                    UNION ALL
                    SELECT * FROM mempool_tx
                )
                SELECT
                    tx_hash as "tx_hash!",
                    block_number as "block_number?",
                    success as "success?",
                    fail_reason as "fail_reason?",
                    eth_block as "eth_block?",
                    priority_op_serialid as "priority_op_serialid?"
                FROM everything
            "#,
            hash,
            &hash_str
        )
        .fetch_optional(transaction.conn())
        .await?;

        let result = if let Some(receipt) = receipt {
            let is_block_finalized =
                is_block_finalized(&mut transaction, receipt.block_number).await?;

            Some(StorageTxReceipt::receipt_from_storage_receipt(
                receipt,
                is_block_finalized,
            ))
        } else {
            None
        };

        transaction.commit().await?;
        metrics::histogram!(
            "sql.chain.operations_ext.tx_receipt_api_v02",
            start.elapsed()
        );
        Ok(result)
    }

    pub async fn tx_data_by_block_and_index_api_v02(
        &mut self,
        block_number: BlockNumber,
        block_index: u64,
    ) -> QueryResult<Option<TxData>> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;
        let data: Option<StorageTxData> = sqlx::query_as!(
            StorageTxData,
            r#"
                WITH transaction AS (
                    SELECT
                        tx_hash,
                        tx as op,
                        block_number,
                        block_index,
                        created_at,
                        success,
                        fail_reason,
                        Null::bytea as eth_hash,
                        Null::bigint as priority_op_serialid,
                        batch_id,
                        eth_sign_data
                    FROM executed_transactions
                    WHERE block_number = $1 AND block_index = $2
                ), priority_op AS (
                    SELECT
                        tx_hash,
                        operation as op,
                        block_number,
                        block_index,
                        created_at,
                        true as success,
                        Null as fail_reason,
                        eth_hash,
                        priority_op_serialid,
                        Null::bigint as batch_id,
                        Null::jsonb as eth_sign_data
                    FROM executed_priority_operations
                    WHERE block_number = $1 AND block_index = $2
                ), 
                everything AS (
                    SELECT * FROM transaction
                    UNION ALL
                    SELECT * FROM priority_op
                )
                SELECT
                    tx_hash as "tx_hash!",
                    op as "op!",
                    block_number as "block_number?",
                    block_index as "block_index?",
                    created_at as "created_at!",
                    success as "success?",
                    fail_reason as "fail_reason?",
                    eth_hash as "eth_hash?",
                    priority_op_serialid as "priority_op_serialid?",
                    batch_id as "batch_id?",
                    eth_sign_data as "eth_sign_data?"
                FROM everything
            "#,
            block_number.0 as i32,
            block_index as i32
        )
        .fetch_optional(transaction.conn())
        .await?;

        let result = if let Some(data) = data {
            Some(tx_data_from_storage(&mut transaction, data).await?)
        } else {
            None
        };

        transaction.commit().await?;
        metrics::histogram!(
            "sql.chain.operations_ext.tx_data_by_block_and_index_api_v02",
            start.elapsed()
        );
        Ok(result)
    }

    pub async fn tx_data_api_v02(&mut self, hash: &[u8]) -> QueryResult<Option<TxData>> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;
        let hash_str = hex::encode(hash);
        let data: Option<StorageTxData> = sqlx::query_as!(
            StorageTxData,
            r#"
                WITH transaction AS (
                    SELECT
                        tx_hash,
                        tx as op,
                        block_number,
                        block_index,
                        created_at,
                        success,
                        fail_reason,
                        Null::bytea as eth_hash,
                        Null::bigint as priority_op_serialid,
                        batch_id,
                        eth_sign_data
                    FROM executed_transactions
                    WHERE tx_hash = $1
                ), priority_op AS (
                    SELECT
                        tx_hash,
                        operation as op,
                        block_number,
                        block_index,
                        created_at,
                        true as success,
                        Null as fail_reason,
                        eth_hash,
                        priority_op_serialid,
                        Null::bigint as batch_id,
                        Null::jsonb as eth_sign_data
                    FROM executed_priority_operations
                    WHERE tx_hash = $1 OR eth_hash = $1
                ), mempool_tx AS (
                    SELECT
                        decode(tx_hash, 'hex'),
                        tx as op,
                        Null::bigint as block_number,
                        Null::int as block_index,
                        created_at,
                        Null::boolean as success,
                        Null as fail_reason,
                        Null::bytea as eth_hash,
                        Null::bigint as priority_op_serialid,
                        batch_id,
                        eth_sign_data
                    FROM mempool_txs
                    WHERE tx_hash = $2
                ),
                everything AS (
                    SELECT * FROM transaction
                    UNION ALL
                    SELECT * FROM priority_op
                    UNION ALL
                    SELECT * FROM mempool_tx
                )
                SELECT
                    tx_hash as "tx_hash!",
                    op as "op!",
                    block_number as "block_number?",
                    block_index as "block_index?",
                    created_at as "created_at!",
                    success as "success?",
                    fail_reason as "fail_reason?",
                    eth_hash as "eth_hash?",
                    priority_op_serialid as "priority_op_serialid?",
                    batch_id as "batch_id?",
                    eth_sign_data as "eth_sign_data?"
                FROM everything
            "#,
            hash,
            &hash_str
        )
        .fetch_optional(transaction.conn())
        .await?;

        let result = if let Some(data) = data {
            Some(tx_data_from_storage(&mut transaction, data).await?)
        } else {
            None
        };

        transaction.commit().await?;
        metrics::histogram!("sql.chain.operations_ext.tx_data_api_v02", start.elapsed());
        Ok(result)
    }

    pub async fn get_priority_op_receipt(
        &mut self,
        op_id: u32,
    ) -> QueryResult<PriorityOpReceiptResponse> {
        let start = Instant::now();
        let stored_executed_prior_op = OperationsSchema(self.0)
            .get_executed_priority_operation(op_id)
            .await?;

        let result = match stored_executed_prior_op {
            Some(stored_executed_prior_op) => {
                let verified = OperationsSchema(self.0)
                    .get_stored_aggregated_operation(
                        BlockNumber(stored_executed_prior_op.block_number as u32),
                        AggregatedActionType::ExecuteBlocks,
                    )
                    .await
                    .map(|operation| operation.confirmed)
                    .unwrap_or_default();

                Ok(PriorityOpReceiptResponse {
                    committed: true,
                    verified,
                    prover_run: None,
                })
            }
            None => Ok(PriorityOpReceiptResponse {
                committed: false,
                verified: false,
                prover_run: None,
            }),
        };

        metrics::histogram!(
            "sql.chain.operations_ext.get_priority_op_receipt",
            start.elapsed()
        );
        result
    }

    pub async fn get_tx_by_hash(&mut self, hash: &[u8]) -> QueryResult<Option<TxByHashResponse>> {
        let start = Instant::now();

        // Attempt to find the transaction in the list of executed operations.
        let result = if let Some(response) = self.find_tx_by_hash(hash).await? {
            Some(response)
        } else {
            // If the transaction is not found in the list of executed operations check executed priority operations list.
            self.find_priority_op_by_hash(hash).await?
        };

        metrics::histogram!("sql.chain.operations_ext.get_tx_by_hash", start.elapsed());
        Ok(result)
    }

    /// Helper method for `get_tx_by_hash` which attempts to find a transaction
    /// in the list of executed operations.
    async fn find_tx_by_hash(&mut self, hash: &[u8]) -> QueryResult<Option<TxByHashResponse>> {
        let start = Instant::now();
        // TODO: Maybe move the transformations to api_server (ZKS-114)?
        let query_result = OperationsSchema(self.0)
            .get_executed_operation(hash)
            .await?;

        let result = if let Some(tx) = query_result {
            let block_number = tx.block_number;
            let fail_reason = tx.fail_reason.clone();
            let created_at = tx.created_at.format("%Y-%m-%dT%H:%M:%S%.6f").to_string();
            let operation = &tx.tx;

            let tx_type = operation["type"].as_str().unwrap_or("unknown tx_type");
            let nonce = operation["nonce"].as_i64().unwrap_or(-1);

            let (tx_from, tx_to, tx_fee, tx_amount, tx_token) = match tx_type {
                "Withdraw" | "Transfer" | "TransferToNew" => (
                    operation["from"]
                        .as_str()
                        .unwrap_or("unknown from")
                        .to_string(),
                    operation["to"].as_str().unwrap_or("unknown to").to_string(),
                    operation["fee"].as_str().map(|v| v.to_string()),
                    operation["amount"]
                        .as_str()
                        .unwrap_or("unknown amount")
                        .to_string(),
                    operation["token"].as_i64().unwrap_or(-1),
                ),
                "ChangePubKey" | "ChangePubKeyOffchain" => (
                    operation["account"]
                        .as_str()
                        .unwrap_or("unknown from")
                        .to_string(),
                    operation["newPkHash"]
                        .as_str()
                        .unwrap_or("unknown to")
                        .to_string(),
                    operation["fee"].as_str().map(|v| v.to_string()),
                    "unknown amount".to_string(),
                    operation["feeToken"].as_i64().unwrap_or(-1),
                ),
                "MintNFT" => (
                    operation["creatorAddress"]
                        .as_str()
                        .unwrap_or("unknown from")
                        .to_string(),
                    operation["recipient"]
                        .as_str()
                        .unwrap_or("unknown to")
                        .to_string(),
                    operation["fee"].as_str().map(|v| v.to_string()),
                    "1".to_string(),
                    operation["feeToken"].as_i64().unwrap_or(-1),
                ),
                "WithdrawNFT" => (
                    operation["from"]
                        .as_str()
                        .unwrap_or("unknown from")
                        .to_string(),
                    operation["to"].as_str().unwrap_or("unknown to").to_string(),
                    operation["fee"].as_str().map(|v| v.to_string()),
                    "1".to_string(),
                    operation["token"].as_i64().unwrap_or(-1),
                ),
                "ForcedExit" => (
                    operation["target"]
                        .as_str()
                        .unwrap_or("unknown from")
                        .to_string(),
                    operation["target"]
                        .as_str()
                        .unwrap_or("unknown to")
                        .to_string(),
                    operation["fee"].as_str().map(|v| v.to_string()),
                    tx.operation["withdraw_amount"]
                        .as_str()
                        .unwrap_or("unknown amount")
                        .to_string(),
                    operation["token"].as_i64().unwrap_or(-1),
                ),
                "Swap" => (
                    operation["submitterAddress"]
                        .as_str()
                        .unwrap_or("unknown from")
                        .to_string(),
                    operation["submitterAddress"]
                        .as_str()
                        .unwrap_or("unknown to")
                        .to_string(),
                    operation["fee"].as_str().map(|v| v.to_string()),
                    "0".to_string(),
                    operation["feeToken"].as_i64().unwrap_or(-1),
                ),
                &_ => (
                    "unknown from".to_string(),
                    "unknown to".to_string(),
                    Some("unknown fee".to_string()),
                    "unknown amount".to_string(),
                    operation["token"].as_i64().unwrap_or(-1),
                ),
            };

            let tx_type_user = if tx_type == "TransferToNew" {
                "Transfer"
            } else {
                tx_type
            };

            Some(TxByHashResponse {
                tx_type: tx_type_user.to_string(),
                from: tx_from,
                to: tx_to,
                token: tx_token as i32,
                amount: tx_amount,
                fee: tx_fee,
                block_number,
                nonce,
                created_at,
                fail_reason,
                tx: tx.tx,
                batch_id: tx.batch_id,
            })
        } else {
            None
        };

        metrics::histogram!("sql.chain.operations_ext.find_tx_by_hash", start.elapsed());
        Ok(result)
    }

    /// Helper method for `get_tx_by_hash` which attempts to find a transaction
    /// in the list of executed priority operations.
    async fn find_priority_op_by_hash(
        &mut self,
        hash: &[u8],
    ) -> QueryResult<Option<TxByHashResponse>> {
        let start = Instant::now();
        // TODO: Maybe move the transformations to api_server (ZKS-114)?
        let tx: Option<StoredExecutedPriorityOperation> = OperationsSchema(self.0)
            .get_executed_priority_operation_by_eth_hash(hash)
            .await?;

        let result = if let Some(tx) = tx {
            let operation = tx.operation;
            let block_number = tx.block_number;
            let created_at = tx.created_at.format("%Y-%m-%dT%H:%M:%S%.6f").to_string();

            let tx_type = operation["type"].as_str().unwrap_or("unknown type");
            let tx_token = operation["priority_op"]["token"]
                .as_i64()
                .expect("must be here");

            let (tx_from, tx_to, tx_fee, tx_amount) = match tx_type {
                "Deposit" => (
                    operation["priority_op"]["from"]
                        .as_str()
                        .unwrap_or("unknown from")
                        .to_string(),
                    operation["priority_op"]["to"]
                        .as_str()
                        .unwrap_or("unknown to")
                        .to_string(),
                    None,
                    operation["priority_op"]["amount"]
                        .as_str()
                        .unwrap_or("unknown amount"),
                ),
                "FullExit" => (
                    operation["priority_op"]["eth_address"]
                        .as_str()
                        .unwrap_or("unknown from")
                        .to_string(),
                    operation["priority_op"]["eth_address"]
                        .as_str()
                        .unwrap_or("unknown to")
                        .to_string(),
                    None,
                    operation["withdraw_amount"]
                        .as_str()
                        .unwrap_or("unknown amount"),
                ),
                &_ => (
                    "unknown from".to_string(),
                    "unknown to".to_string(),
                    Some("unknown fee".to_string()),
                    "unknown amount",
                ),
            };

            Some(TxByHashResponse {
                tx_type: tx_type.to_string(),
                from: tx_from,
                to: tx_to,
                token: tx_token as i32,
                amount: tx_amount.to_string(),
                fee: tx_fee,
                block_number,
                nonce: -1,
                created_at,
                fail_reason: None,
                tx: operation,
                batch_id: None,
            })
        } else {
            None
        };

        metrics::histogram!(
            "sql.chain.operations_ext.find_priority_op_by_hash",
            start.elapsed()
        );
        Ok(result)
    }

    /// Loads the date and time of the moment when the first transaction for the account was executed.
    /// Can be `None` if there were no transactions associated with provided address.
    pub async fn account_created_on(
        &mut self,
        address: &Address,
    ) -> QueryResult<Option<DateTime<Utc>>> {
        let start = Instant::now();
        // This query loads the `committed_at` field from both `executed_transactions` and
        // `executed_priority_operations` tables and returns the oldest result.
        let first_history_entry = sqlx::query_as!(
            AccountCreatedAt,
            r#"
            select 
                created_at as "created_at!"
            from (
                    select
                        created_at
                    from
                        executed_transactions
                    where
                        from_account = $1
                        or
                        to_account = $1
                        or
                        primary_account_address = $1
                    union all
                    select
                        created_at
                    from 
                        executed_priority_operations
                    where 
                        from_account = $1
                        or
                        to_account = $1
            ) t
            order by
                created_at asc
            limit 
                1
            "#,
            address.as_ref(),
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.operations_ext.account_created_on",
            start.elapsed()
        );
        Ok(first_history_entry.map(|entry| entry.created_at))
    }

    /// Loads the range of the transactions applied to the account starting
    /// from the block with number $(offset) up to $(offset + limit).
    pub async fn get_account_transactions_history(
        &mut self,
        address: &Address,
        offset: u64,
        limit: u64,
    ) -> QueryResult<Vec<TransactionsHistoryItem>> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        // This query does the following:
        // - creates a union of `executed_transactions` and the `executed_priority_operations`
        // - unifies the information to match the `TransactionsHistoryItem`
        //   structure layout
        // - returns the obtained results.
        //
        // Additional note:
        // - previously for "committed" flag we've checked the operation "confirmed" field the
        //   same way as it done for "verified" flag. Later we've decided that if tx was added
        //   to the `executed_*` table, it actually **is** committed, thus now we just add
        //   `true`.
        let mut tx_history = sqlx::query_as!(
            TransactionsHistoryItem,
            r#"
            WITH aggr_exec AS (
                SELECT
                    aggregate_operations.confirmed,
                    execute_aggregated_blocks_binding.block_number
                FROM aggregate_operations
                    INNER JOIN execute_aggregated_blocks_binding ON aggregate_operations.id = execute_aggregated_blocks_binding.op_id
                WHERE aggregate_operations.confirmed = true
            ), tx_hashes AS (
                SELECT DISTINCT sequence_number FROM tx_filters
                WHERE address = $1
                ORDER BY sequence_number desc
                OFFSET $2
                LIMIT $3
            ), transactions AS (
                SELECT
                    *
                FROM (
                    SELECT
                        concat_ws(',', block_number, block_index) AS tx_id,
                        tx,
                        'sync-tx:' || encode(executed_transactions.tx_hash, 'hex') AS hash,
                        null as pq_id,
                        null as eth_block,
                        success,
                        fail_reason,
                        block_number,
                        created_at,
                        executed_transactions.sequence_number,
                        batch_id
                    FROM executed_transactions
                    INNER JOIN tx_hashes
                        ON tx_hashes.sequence_number = executed_transactions.sequence_number
                    UNION ALL
                    SELECT
                        concat_ws(',', block_number, block_index) AS tx_id,
                        operation as tx,
                        '0x' || encode(eth_hash, 'hex') as hash,
                        priority_op_serialid as pq_id,
                        eth_block,
                        true as success,
                        null as fail_reason,
                        block_number,
                        created_at,
                        executed_priority_operations.sequence_number,
                        Null::bigint as batch_id
                    FROM executed_priority_operations 
                    INNER JOIN tx_hashes
                        ON tx_hashes.sequence_number = executed_priority_operations.sequence_number
                    ) t
            )
            SELECT
                tx_id as "tx_id!",
                hash as "hash?",
                eth_block as "eth_block?",
                pq_id as "pq_id?",
                tx as "tx!",
                success as "success?",
                fail_reason as "fail_reason?",
                true as "commited!",
                coalesce(verified.confirmed, false) as "verified!",
                created_at as "created_at!",
                batch_id as "batch_id?"
            FROM transactions
            LEFT JOIN aggr_exec verified ON transactions.block_number = verified.block_number
            ORDER BY transactions.block_number DESC, sequence_number DESC
            "#,
            address.as_ref(), offset as i64, limit as i64
        ).fetch_all(transaction.conn())
        .await?;

        if !tx_history.is_empty() {
            let tokens = transaction.tokens_schema().load_tokens().await?;
            for tx_item in &mut tx_history {
                let tx_info = match tx_item.tx["type"].as_str().unwrap_or("NONE") {
                    "NONE" => {
                        vlog::warn!("Tx history item type not found, tx: {:?}", tx_item);
                        continue;
                    }
                    "Deposit" | "FullExit" => tx_item.tx.get_mut("priority_op"),
                    _ => Some(&mut tx_item.tx),
                };

                let tx_info = if let Some(tx_info) = tx_info {
                    tx_info
                } else {
                    vlog::warn!("tx_info not found for tx: {:?}", tx_item);
                    continue;
                };

                if let Some(tok_val) = tx_info.get_mut("token") {
                    if let Some(token_id) = tok_val.as_u64() {
                        if token_id < params::MIN_NFT_TOKEN_ID as u64 {
                            let token_id = TokenId(token_id as u32);
                            let token_symbol = tokens
                                .get(&token_id)
                                .map(|t| t.symbol.clone())
                                .unwrap_or_else(|| "UNKNOWN".to_string());
                            *tok_val =
                                serde_json::to_value(token_symbol).expect("json string to value");
                        } else {
                            *tok_val =
                                serde_json::to_value(token_id).expect("json string to value");
                        }
                    };
                };
            }
        }

        transaction.commit().await?;
        metrics::histogram!(
            "sql.chain.operations_ext.get_account_transactions_history",
            start.elapsed()
        );
        Ok(tx_history)
    }

    async fn get_closest_sequence_number(
        &mut self,
        block_number: i64,
        block_index: i32,
        direction: SearchDirection,
    ) -> QueryResult<Option<i64>> {
        if block_number == 0 {
            return Ok(Some(0));
        }
        let (function, function2) = match direction {
            SearchDirection::Older => ("MAX", "GREATEST"),
            SearchDirection::Newer => ("MIN", "LEAST"),
        };
        let query = format!(
            r#"
            SELECT COALESCE(
                (SELECT {1}(
                    (
                        SELECT {0}(sequence_number) as sequence_number
                        FROM executed_transactions
                        WHERE block_number=$1 AND block_index=$2
                    ),
                    (
                        SELECT {0}(sequence_number) AS sequence_number
                        FROM executed_priority_operations
                        WHERE block_number=$1 AND block_index=$2
                    )
                )),
                (SELECT {1}(
                    (SELECT {0}(sequence_number) as sequence_number FROM executed_transactions WHERE block_number=$1),
                    (SELECT {0}(sequence_number) as sequence_number FROM executed_priority_operations WHERE block_number=$1)
                ) + 1),
                (SELECT {0}(sequence_number) + 1 FROM tx_filters)
            )
            "#,
            function, function2
        );
        let tx_seq_no: Option<i64> = sqlx::query_scalar(&query)
            .bind(block_number as i32)
            .bind(block_index)
            .fetch_one(self.0.conn())
            .await?;
        Ok(tx_seq_no)
    }

    /// Loads the range of the transactions applied to the account starting
    /// from the specified transaction ID.
    ///
    /// This method can be used to get transactions "older" than some transaction
    /// or "newer" than one.
    ///
    /// Unlike `get_account_transactions_history`, this method does not use
    /// a relative offset, and thus not prone to report the same tx twice if new
    /// transactions were added to the database.
    pub async fn get_account_transactions_history_from(
        &mut self,
        address: &Address,
        tx_id: (u64, u64),
        direction: SearchDirection,
        limit: u64,
    ) -> QueryResult<Vec<TransactionsHistoryItem>> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let (block_id, block_tx_id) = tx_id;
        let sequence_number = transaction
            .chain()
            .operations_ext_schema()
            .get_closest_sequence_number(block_id as i64, block_tx_id as i32, direction)
            .await?;
        let sequence_number = if let Some(sequence_number) = sequence_number {
            sequence_number
        } else {
            // If the tx with provided data doesn't exist we return empty vector
            return Ok(vec![]);
        };

        let (pagination_query, order_query) = match direction {
            SearchDirection::Older => (
                "SELECT DISTINCT sequence_number FROM tx_filters
                WHERE address = $1
                AND sequence_number < $2
                ORDER BY sequence_number DESC 
                LIMIT $3
                ",
                "ORDER BY transactions.block_number DESC, transactions.sequence_number DESC",
            ),
            SearchDirection::Newer => (
                "
                SELECT DISTINCT sequence_number FROM tx_filters
                WHERE address = $1
                AND sequence_number > $2
                ORDER BY sequence_number DESC
                LIMIT $3
                ",
                "ORDER BY transactions.block_number DESC, transactions.sequence_number DESC",
            ),
        };

        // This query does the following:
        // - Paginate txs using tx_filters table
        // - creates a union of `executed_transactions` and the `executed_priority_operations`
        // - unifies the information to match the `TransactionsHistoryItem`
        //   structure layout
        // - returns the obtained results.
        //
        // Additional note:
        // - previously for "committed" flag we've checked the operation "confirmed" field the
        //   same way as it done for "verified" flag. Later we've decided that if tx was added
        //   to the `executed_*` table, it actually **is** committed, thus now we just add
        //   `true`.
        let query = format!(
            r#"
            WITH aggr_comm AS (
                SELECT
                   aggregate_operations.confirmed,
                   commit_aggregated_blocks_binding.block_number
               FROM aggregate_operations
                   INNER JOIN commit_aggregated_blocks_binding ON aggregate_operations.id = commit_aggregated_blocks_binding.op_id
               WHERE aggregate_operations.confirmed = true
            ), aggr_exec AS (
                SELECT
                   aggregate_operations.confirmed,
                   execute_aggregated_blocks_binding.block_number
               FROM aggregate_operations
                   INNER JOIN execute_aggregated_blocks_binding ON aggregate_operations.id = execute_aggregated_blocks_binding.op_id
               WHERE aggregate_operations.confirmed = true
            ), tx_hashes AS (
                {}
            ), transactions AS (
                SELECT
                    *
                FROM (
                    SELECT
                        concat_ws(',', block_number, block_index) AS tx_id,
                        tx,
                        'sync-tx:' || encode(executed_transactions.tx_hash, 'hex') as hash,
                        null as pq_id,
                        null as eth_block,
                        success,
                        fail_reason,
                        block_number,
                        created_at,
                        executed_transactions.sequence_number,
                        batch_id
                    FROM executed_transactions
                    INNER JOIN tx_hashes
                        ON tx_hashes.sequence_number = executed_transactions.sequence_number
                    UNION ALL
                    SELECT
                        concat_ws(',', block_number, block_index) AS tx_id,
                        operation as tx,
                        '0x' || encode(eth_hash, 'hex') as hash,
                        priority_op_serialid as pq_id,
                        eth_block,
                        true as success,
                        null as fail_reason,
                        block_number,
                        created_at,
                        executed_priority_operations.sequence_number,
                        Null::bigint as batch_id
                    FROM
                        executed_priority_operations
                    INNER JOIN tx_hashes
                        ON tx_hashes.sequence_number = executed_priority_operations.sequence_number
                    ) t
            )
            SELECT
                tx_id,
                hash,
                eth_block,
                pq_id,
                tx,
                success,
                fail_reason,
                true as commited,
                coalesce(verified.confirmed, false) as verified,
                created_at ,
                batch_id 
            FROM transactions
            LEFT JOIN aggr_comm committed ON
                committed.block_number = transactions.block_number AND committed.confirmed = true
            LEFT JOIN aggr_exec verified ON
                verified.block_number = transactions.block_number AND verified.confirmed = true
            {}
            "#,
            pagination_query, order_query
        );

        let mut tx_history: Vec<TransactionsHistoryItem> = sqlx::query_as(&query)
            .bind(address.as_bytes())
            .bind(sequence_number)
            .bind(limit as i64)
            .fetch_all(transaction.conn())
            .await?;

        if !tx_history.is_empty() {
            let tokens = transaction.tokens_schema().load_tokens().await?;
            for tx_item in &mut tx_history {
                let tx_info = match tx_item.tx["type"].as_str().unwrap_or("NONE") {
                    "NONE" => {
                        vlog::warn!("Tx history item type not found, tx: {:?}", tx_item);
                        continue;
                    }
                    "Deposit" | "FullExit" => tx_item.tx.get_mut("priority_op"),
                    _ => Some(&mut tx_item.tx),
                };

                let tx_info = if let Some(tx_info) = tx_info {
                    tx_info
                } else {
                    vlog::warn!("tx_info not found for tx: {:?}", tx_item);
                    continue;
                };

                if let Some(tok_val) = tx_info.get_mut("token") {
                    if let Some(token_id) = tok_val.as_u64() {
                        if token_id < params::MIN_NFT_TOKEN_ID as u64 {
                            let token_id = TokenId(token_id as u32);
                            let token_symbol = tokens
                                .get(&token_id)
                                .map(|t| t.symbol.clone())
                                .unwrap_or_else(|| "UNKNOWN".to_string());
                            *tok_val =
                                serde_json::to_value(token_symbol).expect("json string to value");
                        } else {
                            *tok_val =
                                serde_json::to_value(token_id).expect("json string to value");
                        }
                    };
                };
            }
        }

        transaction.commit().await?;
        metrics::histogram!(
            "sql.chain.operations_ext.get_account_transactions_history_from",
            start.elapsed()
        );
        Ok(tx_history)
    }

    pub async fn get_account_transactions(
        &mut self,
        query: &PaginationQuery<AccountTxsRequest>,
    ) -> QueryResult<Option<Vec<Transaction>>> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;
        let tx_hash = match query.from.tx_hash.inner {
            Either::Left(tx_hash) => tx_hash,
            Either::Right(_) => {
                if let Some(tx_hash) = transaction
                    .chain()
                    .operations_ext_schema()
                    .get_account_last_tx_hash(query.from.address)
                    .await?
                {
                    tx_hash
                } else {
                    return Ok(Some(Vec::new()));
                }
            }
        };
        let sequence_number = transaction
            .chain()
            .operations_ext_schema()
            .get_tx_sequence_number(tx_hash)
            .await?;

        let txs = if let Some(id_from) = sequence_number {
            let raw_txs = if let Some(address) = query.from.second_address {
                // It's impossible to have priority operations for two accounts
                transaction
                    .chain()
                    .operations_ext_schema()
                    .get_executed_transactions_for_two_accounts(
                        query.from.address,
                        address,
                        query.from.token,
                        i64::from(query.limit),
                        id_from,
                        query.direction,
                    )
                    .await?
            } else {
                let mut priority_seq_numbers = vec![];
                let mut executed_sequence_numbers = vec![];
                transaction
                    .chain()
                    .operations_ext_schema()
                    .get_tx_seq_numbers_for_account(
                        query.from.address,
                        query.from.token,
                        i64::from(query.limit),
                        id_from,
                        query.direction,
                    )
                    .await?
                    .iter()
                    .for_each(|record| {
                        if record.is_priority {
                            priority_seq_numbers.push(record.sequence_number)
                        } else {
                            executed_sequence_numbers.push(record.sequence_number)
                        }
                    });

                let mut txs = transaction
                    .chain()
                    .operations_ext_schema()
                    .get_executed_txs_for_account(executed_sequence_numbers)
                    .await?;

                txs.append(
                    &mut transaction
                        .chain()
                        .operations_ext_schema()
                        .get_priority_operations_for_account(priority_seq_numbers)
                        .await?,
                );

                txs.into_iter()
                    .sorted_by(|tx1, tx2| match query.direction {
                        PaginationDirection::Newer => tx1.created_at.cmp(&tx2.created_at),
                        PaginationDirection::Older => tx2.created_at.cmp(&tx1.created_at),
                    })
                    .collect()
            };

            let last_finalized = transaction
                .chain()
                .block_schema()
                .get_last_verified_confirmed_block()
                .await?;
            let txs: Vec<Transaction> = raw_txs
                .into_iter()
                .map(|tx| {
                    if tx.block_number as u32 <= *last_finalized {
                        TransactionItem::transaction_from_item(tx, true)
                    } else {
                        TransactionItem::transaction_from_item(tx, false)
                    }
                })
                .collect();
            Some(txs)
        } else {
            None
        };
        transaction.commit().await?;

        metrics::histogram!(
            "sql.chain.operations_ext.get_account_transactions",
            start.elapsed()
        );
        Ok(txs)
    }

    async fn get_executed_transactions_for_two_accounts(
        &mut self,
        address: Address,
        second_address: Address,
        token: Option<TokenId>,
        limit: i64,
        id_from: i64,
        direction: PaginationDirection,
    ) -> QueryResult<Vec<TransactionItem>> {
        let query_direction = match direction {
            PaginationDirection::Newer => {
                "WHERE sequence_number >= $4 
                ORDER BY sequence_number 
                LIMIT $5"
            }
            PaginationDirection::Older => {
                "WHERE sequence_number <= $4
                ORDER BY sequence_number DESC
                LIMIT $5"
            }
        };

        let token_query = if token.is_some() {
            "AND token = $3"
        } else {
            ""
        };

        let query = format!(
            r#"
                WITH tx_hashes AS (
                    SELECT DISTINCT tx_hash FROM tx_filters
                    WHERE address = $1 {} 
                    INTERSECT
                    SELECT DISTINCT tx_hash FROM tx_filters
                    WHERE address = $2 {}
                )
                SELECT                     
                    executed_transactions.tx_hash,
                    sequence_number,
                    tx as op,
                    block_number,
                    created_at,
                    success,
                    fail_reason,
                    Null::bytea as eth_hash,
                    Null::bigint as priority_op_serialid,
                    block_index,
                    batch_id
                FROM tx_hashes INNER JOIN executed_transactions 
                    ON tx_hashes.tx_hash = executed_transactions.tx_hash
                {}
                
            "#,
            token_query, token_query, query_direction
        );

        Ok(sqlx::query_as(&query)
            .bind(address.as_bytes())
            .bind(second_address.as_bytes())
            .bind(token.unwrap_or_default().0 as i32)
            .bind(id_from)
            .bind(limit)
            .fetch_all(self.0.conn())
            .await?)
    }

    async fn get_tx_seq_numbers_for_account(
        &mut self,
        address: Address,
        token: Option<TokenId>,
        limit: i64,
        id_from: i64,
        direction: PaginationDirection,
    ) -> QueryResult<Vec<SequenceNumberRecord>> {
        let query_direction = match direction {
            PaginationDirection::Newer => {
                "AND sequence_number  >= $3
                ORDER BY sequence_number
                LIMIT $4"
            }
            PaginationDirection::Older => {
                "AND sequence_number <= $3
                ORDER BY sequence_number DESC
                LIMIT $4"
            }
        };

        let token_query = if token.is_some() {
            "AND token = $2"
        } else {
            ""
        };

        let query = format!(
            "SELECT DISTINCT sequence_number, is_priority FROM tx_filters WHERE address = $1 {} {}",
            token_query, query_direction
        );

        Ok(sqlx::query_as(&query)
            .bind(address.as_bytes())
            .bind(token.unwrap_or_default().0 as i32)
            .bind(id_from)
            .bind(limit)
            .fetch_all(self.0.conn())
            .await?)
    }

    async fn get_priority_operations_for_account(
        &mut self,
        sequence_numbers: Vec<i64>,
    ) -> QueryResult<Vec<TransactionItem>> {
        Ok(sqlx::query_as!(
            TransactionItem,
            r#"
            SELECT 
                sequence_number,
                tx_hash as "tx_hash!",
                operation as "op!",
                block_number as "block_number!",
                created_at as "created_at!",
                true as "success!",
                Null as fail_reason,
                eth_hash as "eth_hash?", 
                priority_op_serialid as "priority_op_serialid?",
                block_index as "block_index?",
                Null::bigint as batch_id
            FROM executed_priority_operations 
            WHERE sequence_number IN (SELECT u.sequence_number
                FROM UNNEST ($1::bigint[])
                AS u(sequence_number)
            )
        "#,
            &sequence_numbers
        )
        .fetch_all(self.0.conn())
        .await?)
    }

    async fn get_executed_txs_for_account(
        &mut self,
        sequence_numbers: Vec<i64>,
    ) -> QueryResult<Vec<TransactionItem>> {
        Ok(sqlx::query_as!(
            TransactionItem,
            r#"
               SELECT
                    sequence_number,
                    tx_hash as "tx_hash!",
                    tx as "op!",
                    block_number as "block_number!",
                    created_at as "created_at!",
                    success as "success!",
                    fail_reason,
                    Null::bytea as eth_hash,
                    Null::bigint as priority_op_serialid,
                    block_index,
                    batch_id
                FROM executed_transactions 
            WHERE sequence_number IN (SELECT u.sequence_number
                FROM UNNEST ($1::bigint[])
                AS u(sequence_number)
            )
        "#,
            &sequence_numbers
        )
        .fetch_all(self.0.conn())
        .await?)
    }

    pub async fn get_account_last_tx_hash(
        &mut self,
        address: Address,
    ) -> QueryResult<Option<TxHash>> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let record = sqlx::query!(
            r#"
            SELECT tx_hash as "tx_hash!"
                FROM tx_filters as f
                WHERE address = $1
                ORDER BY sequence_number
                DESC
                LIMIT 1
            "#,
            address.as_bytes()
        )
        .fetch_optional(transaction.conn())
        .await?;

        transaction.commit().await?;
        metrics::histogram!(
            "sql.chain.operations_ext.get_account_last_tx_hash",
            start.elapsed()
        );
        Ok(record.map(|record| TxHash::from_slice(&record.tx_hash).unwrap()))
    }

    pub async fn get_block_last_tx_hash(
        &mut self,
        block_number: BlockNumber,
    ) -> QueryResult<Option<TxHash>> {
        let start = Instant::now();
        let record = sqlx::query!(
            r#"
                WITH transactions AS (
                    SELECT tx_hash, sequence_number
                    FROM executed_transactions
                    WHERE block_number = $1
                ), priority_ops AS (
                    SELECT tx_hash, sequence_number
                    FROM executed_priority_operations
                    WHERE block_number = $1
                ), everything AS (
                    SELECT * FROM transactions
                    UNION ALL
                    SELECT * FROM priority_ops
                )
                SELECT
                    tx_hash as "tx_hash!"
                FROM everything
                ORDER BY sequence_number DESC
                LIMIT 1
            "#,
            i64::from(*block_number)
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.operations_ext.get_block_last_tx_hash",
            start.elapsed()
        );
        Ok(record.map(|record| TxHash::from_slice(&record.tx_hash).unwrap()))
    }

    // TODO Remove it after migration is complete
    pub async fn get_accounts_range(
        &mut self,
        start_account: Option<Address>,
        limit: u32,
    ) -> Option<(Address, Address)> {
        let start_account = match start_account {
            None => {
                let address = sqlx::query_scalar!(
                    r#"
                    SELECT DISTINCT address
                    FROM tx_filters
                    ORDER BY address
                    LIMIT 1
                "#,
                )
                .fetch_one(self.0.conn())
                .await
                .unwrap();
                Address::from_slice(&address)
            }
            Some(account) => account,
        };

        sqlx::query_scalar!(
            r#"
            SELECT * FROM ( 
                SELECT DISTINCT address
                FROM tx_filters
                WHERE address > $1
                ORDER BY address
                LIMIT $2
            ) AS a
            ORDER BY address DESC LIMIT 1
        "#,
            start_account.as_bytes(),
            limit as i32
        )
        .fetch_optional(self.0.conn())
        .await
        .unwrap()
        .map(|account| (start_account, Address::from_slice(&account)))
    }

    // TODO Remove it after migration is complete
    pub async fn update_txs_count(
        &mut self,
        start_account: Address,
        finish_account: Address,
    ) -> QueryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO txs_count (address, token, count)
            SELECT address,token, COUNT(DISTINCT tx_hash)
            FROM tx_filters
            WHERE  address > $1 AND address <= $2
                GROUP BY (address, token)
            ON CONFLICT( address, token) DO UPDATE SET count = EXCLUDED.count;
            "#,
            start_account.as_bytes(),
            finish_account.as_bytes(),
        )
        .execute(self.0.conn())
        .await?;

        sqlx::query!(
            r#"
            INSERT INTO txs_count (address, token, count)
            SELECT address, -1, COUNT(DISTINCT tx_hash)
            FROM tx_filters
            WHERE address > $1 AND address <= $2
                GROUP BY (address)
            ON CONFLICT( address, token) DO UPDATE SET count = EXCLUDED.count;
            "#,
            start_account.as_bytes(),
            finish_account.as_bytes(),
        )
        .execute(self.0.conn())
        .await?;
        Ok(())
    }

    pub async fn get_account_transactions_count(
        &mut self,
        address: Address,
        token: Option<TokenId>,
        second_address: Option<Address>,
    ) -> QueryResult<u32> {
        let start = Instant::now();

        let count = if let Some(second_address) = second_address {
            sqlx::query!(
                r#"
                WITH tx_hashes AS (
                    SELECT DISTINCT tx_hash FROM tx_filters
                    WHERE address = $1 AND ($2::boolean OR token = $3)
                    INTERSECT
                    SELECT DISTINCT tx_hash FROM tx_filters
                    WHERE address = $4 AND ($2::boolean OR token = $3)
                )
                SELECT COUNT(*) as "count!" FROM tx_hashes
                "#,
                address.as_bytes(),
                token.is_none(),
                token.unwrap_or_default().0 as i32,
                second_address.as_bytes()
            )
            .fetch_one(self.0.conn())
            .await?
            .count
        } else {
            // Postgresql doesn't support unique indexes for nullable fields, so we have to use
            // artificial token -1 which means no token
            sqlx::query!(
                r#"
                  SELECT
                    count
                  FROM
                    txs_count
                  WHERE address = $1 
                  AND token = $2
                "#,
                address.as_bytes(),
                token.map(|a| a.0 as i32).unwrap_or(-1)
            )
            .fetch_one(self.0.conn())
            .await?
            .count
        };
        metrics::histogram!(
            "sql.chain.operations_ext.get_account_transactions_count",
            start.elapsed()
        );
        Ok(count as u32)
    }

    /// Returns `created_at` for `block_number` fields for transaction with given hash.
    pub async fn get_tx_sequence_number_for_block(
        &mut self,
        tx_hash: TxHash,
        block_number: BlockNumber,
    ) -> QueryResult<Option<i64>> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let result = sqlx::query!(
            "SELECT sequence_number FROM executed_transactions
            WHERE tx_hash = $1 AND block_number = $2",
            tx_hash.as_ref(),
            block_number.0 as i32
        )
        .fetch_optional(transaction.conn())
        .await?
        .map(|record| record.sequence_number)
        .flatten();

        if result.is_some() {
            return Ok(result);
        }

        // TxHash is not unique for priority operations so we have to get the max created_at
        // because we are using this function for paginating starting from the latest transaction
        let result = sqlx::query!(
            r#"SELECT sequence_number  FROM executed_priority_operations
                WHERE tx_hash = $1 AND block_number = $2 ORDER BY sequence_number DESC"#,
            tx_hash.as_ref(),
            block_number.0 as i32
        )
        .fetch_optional(transaction.conn())
        .await?
        .map(|record| record.sequence_number)
        .flatten();
        transaction.commit().await?;

        metrics::histogram!("sql.chain.block.get_tx_sequence_number", start.elapsed());
        Ok(result)
    }
    /// Returns `created_at` and `block_number` fields for transaction with given hash.
    pub async fn get_tx_sequence_number(&mut self, tx_hash: TxHash) -> QueryResult<Option<i64>> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let record = sqlx::query!(
            "SELECT sequence_number FROM executed_transactions
            WHERE tx_hash = $1",
            tx_hash.as_ref()
        )
        .fetch_optional(transaction.conn())
        .await?;

        let result = if let Some(record) = record {
            Some(record.sequence_number)
        } else {
            // TxHash is not unique for priority operations so we have to get the max created_at
            // because we are using this function for paginating starting from the latest transaction
            let record = sqlx::query!(
                r#"SELECT sequence_number FROM executed_priority_operations
                WHERE tx_hash = $1 ORDER BY sequence_number DESC"#,
                tx_hash.as_ref()
            )
            .fetch_optional(transaction.conn())
            .await?;

            record.map(|record| record.sequence_number)
        }
        .flatten();
        transaction.commit().await?;

        metrics::histogram!("sql.chain.block.get_tx_sequence_number", start.elapsed());
        Ok(result)
    }

    pub async fn get_in_block_batch_info(
        &mut self,
        batch_hash: TxHash,
    ) -> QueryResult<Option<ApiTxBatch>> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let batch_data: Vec<InBlockBatchTx> = sqlx::query_as!(
            InBlockBatchTx,
            r#"
                SELECT tx_hash, created_at, success, block_number
                FROM executed_transactions
                INNER JOIN txs_batches_hashes
                ON txs_batches_hashes.batch_id = COALESCE(executed_transactions.batch_id, 0)
                WHERE batch_hash = $1
                ORDER BY sequence_number ASC
            "#,
            batch_hash.as_ref()
        )
        .fetch_all(transaction.conn())
        .await?;
        let result = if !batch_data.is_empty() {
            let created_at = batch_data[0].created_at;
            let transaction_hashes: Vec<TxHashSerializeWrapper> = batch_data
                .iter()
                .map(|tx| TxHashSerializeWrapper(TxHash::from_slice(&tx.tx_hash).unwrap()))
                .collect();
            let block_number = BlockNumber(batch_data[0].block_number as u32);
            let batch_status = if batch_data[0].success {
                if let Some(op) = transaction
                    .chain()
                    .operations_schema()
                    .get_stored_aggregated_operation(
                        block_number,
                        AggregatedActionType::ExecuteBlocks,
                    )
                    .await
                {
                    BatchStatus {
                        updated_at: op.created_at,
                        last_state: TxInBlockStatus::Finalized,
                    }
                } else {
                    BatchStatus {
                        updated_at: created_at,
                        last_state: TxInBlockStatus::Committed,
                    }
                }
            } else {
                BatchStatus {
                    updated_at: created_at,
                    last_state: TxInBlockStatus::Rejected,
                }
            };
            Some(ApiTxBatch {
                batch_hash,
                transaction_hashes,
                created_at,
                batch_status,
            })
        } else {
            None
        };
        transaction.commit().await?;

        metrics::histogram!("sql.chain.block.get_in_block_batch_info", start.elapsed());
        Ok(result)
    }

    pub async fn get_batch_info(&mut self, batch_hash: TxHash) -> QueryResult<Option<ApiTxBatch>> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let result = if let Some(batch_info) = transaction
            .chain()
            .operations_ext_schema()
            .get_in_block_batch_info(batch_hash)
            .await?
        {
            Some(batch_info)
        } else {
            transaction
                .chain()
                .mempool_schema()
                .get_queued_batch_info(batch_hash)
                .await?
        };
        transaction.commit().await?;

        metrics::histogram!("sql.chain.block.get_batch_info", start.elapsed());
        Ok(result)
    }

    pub async fn tx_data_for_web3(&mut self, hash: &[u8]) -> QueryResult<Option<Web3TxData>> {
        let start = Instant::now();

        let result: Option<Web3TxData> = sqlx::query_as!(
            Web3TxData,
            r#"
                WITH transaction AS (
                    SELECT
                        tx_hash,
                        block_number,
                        nonce,
                        block_index,
                        from_account,
                        to_account
                    FROM executed_transactions
                    WHERE tx_hash = $1
                ), priority_op AS (
                    SELECT
                        tx_hash,
                        block_number,
                        priority_op_serialid as nonce,
                        block_index,
                        from_account,
                        to_account
                    FROM executed_priority_operations
                    WHERE tx_hash = $1 OR eth_hash = $1
                ),
                everything AS (
                    SELECT * FROM transaction
                    UNION ALL
                    SELECT * FROM priority_op
                )
                SELECT
                    tx_hash as "tx_hash!",
                    block_number as "block_number!",
                    nonce as "nonce!",
                    block_index as "block_index?",
                    from_account as "from_account!",
                    to_account as "to_account?",
                    root_hash as "block_hash!"
                FROM everything
                LEFT JOIN blocks
                    ON everything.block_number = blocks.number
            "#,
            hash,
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!("sql.chain.operations_ext.tx_data_for_web3", start.elapsed());
        Ok(result)
    }

    pub async fn web3_receipt_by_hash(
        &mut self,
        hash: &[u8],
    ) -> QueryResult<Option<Web3TxReceipt>> {
        let start = Instant::now();

        let tx: Option<Web3TxReceipt> = sqlx::query_as!(
            Web3TxReceipt,
            r#"
                WITH transaction AS (
                    SELECT
                        tx_hash,
                        block_number,
                        operation,
                        block_index,
                        from_account,
                        to_account,
                        success
                    FROM executed_transactions
                    WHERE tx_hash = $1
                ), priority_op AS (
                    SELECT
                        tx_hash,
                        block_number,
                        operation,
                        block_index,
                        from_account,
                        to_account,
                        true as success
                    FROM executed_priority_operations
                    WHERE tx_hash = $1 OR eth_hash = $1
                ),
                everything AS (
                    SELECT * FROM transaction
                    UNION ALL
                    SELECT * FROM priority_op
                )
                SELECT
                    tx_hash as "tx_hash!",
                    block_number as "block_number!",
                    operation as "operation!",
                    block_index as "block_index?",
                    from_account as "from_account!",
                    to_account as "to_account?",
                    success as "success!",
                    root_hash as "block_hash!"
                FROM everything
                LEFT JOIN blocks
                    ON everything.block_number = blocks.number
                LEFT JOIN aggregate_operations
                    ON (blocks.number BETWEEN aggregate_operations.from_block AND aggregate_operations.to_block)
                    AND aggregate_operations.action_type = 'CommitBlocks'
                WHERE confirmed = true
            "#,
            hash
        )
            .fetch_optional(self.0.conn())
            .await?;

        metrics::histogram!(
            "sql.chain.operations_ext.web3_receipt_by_hash",
            start.elapsed()
        );
        Ok(tx)
    }

    pub async fn web3_receipts(
        &mut self,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> QueryResult<Vec<Web3TxReceipt>> {
        let start = Instant::now();

        let receipts: Vec<Web3TxReceipt> = sqlx::query_as!(
            Web3TxReceipt,
            r#"
                WITH transaction AS (
                    SELECT
                        tx_hash,
                        block_number,
                        operation,
                        block_index,
                        from_account,
                        to_account,
                        success
                    FROM executed_transactions
                    WHERE block_number BETWEEN $1 AND $2
                ), priority_op AS (
                    SELECT
                        tx_hash,
                        block_number,
                        operation,
                        block_index,
                        from_account,
                        to_account,
                        true as success
                    FROM executed_priority_operations
                    WHERE block_number BETWEEN $1 AND $2
                ),
                everything AS (
                    SELECT * FROM transaction
                    UNION ALL
                    SELECT * FROM priority_op
                )
                SELECT
                    tx_hash as "tx_hash!",
                    block_number as "block_number!",
                    operation as "operation!",
                    block_index as "block_index?",
                    from_account as "from_account!",
                    to_account as "to_account?",
                    success as "success!",
                    root_hash as "block_hash!"
                FROM everything
                LEFT JOIN blocks
                    ON everything.block_number = blocks.number
                LEFT JOIN aggregate_operations
                    ON (blocks.number BETWEEN aggregate_operations.from_block AND aggregate_operations.to_block)
                    AND aggregate_operations.action_type = 'CommitBlocks'
                WHERE confirmed = true
            "#,
            i64::from(from_block.0),
            i64::from(to_block.0)
        )
            .fetch_all(self.0.conn())
            .await?;

        metrics::histogram!("sql.chain.operations_ext.web3_receipts", start.elapsed());
        Ok(receipts)
    }

    pub async fn load_executed_txs_in_block_range(
        &mut self,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> QueryResult<Vec<(H256, ZkSyncTx)>> {
        let records = sqlx::query!(
            "SELECT tx_hash, tx FROM executed_transactions WHERE block_number BETWEEN $1 AND $2",
            from_block.0 as i64,
            to_block.0 as i64
        )
        .fetch_all(self.0.conn())
        .await?;
        let result = records
            .into_iter()
            .map(|record| {
                (
                    H256::from_slice(&record.tx_hash),
                    serde_json::from_value(record.tx).unwrap(),
                )
            })
            .collect();
        Ok(result)
    }

    pub async fn load_executed_priority_ops_in_block_range(
        &mut self,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> QueryResult<Vec<(H256, ZkSyncOp)>> {
        let records = sqlx::query!(
            "SELECT tx_hash, operation FROM executed_priority_operations WHERE block_number BETWEEN $1 AND $2",
            from_block.0 as i64,
            to_block.0 as i64
        )
            .fetch_all(self.0.conn())
            .await?;
        let result = records
            .into_iter()
            .map(|record| {
                (
                    H256::from_slice(&record.tx_hash),
                    serde_json::from_value(record.operation).unwrap(),
                )
            })
            .collect();
        Ok(result)
    }

    pub async fn last_block_with_updated_tx_filters(&mut self) -> QueryResult<BlockNumber> {
        let max1: i64 = sqlx::query!(
            r#"
                SELECT MAX(block_number) as "max?" FROM tx_filters
                INNER JOIN executed_transactions
                ON tx_filters.tx_hash = executed_transactions.tx_hash
            "#
        )
        .fetch_one(self.0.conn())
        .await?
        .max
        .unwrap_or_default();
        let max2: i64 = sqlx::query!(
            r#"
                SELECT MAX(block_number) as "max?" FROM tx_filters
                INNER JOIN executed_priority_operations
                ON tx_filters.tx_hash = executed_priority_operations.tx_hash
            "#
        )
        .fetch_one(self.0.conn())
        .await?
        .max
        .unwrap_or_default();

        Ok(BlockNumber(std::cmp::max(max1, max2) as u32))
    }

    // TODO Delete it right after execution
    pub async fn set_unique_sequence_number_for_priority_operations(
        &mut self,
        last_seq_no: i64,
        excluded_tx_hashes: &[Vec<u8>],
    ) -> i64 {
        let values = sqlx::query!(
            r#"
            SELECT sequence_number, tx_hash 
            FROM executed_priority_operations 
            WHERE sequence_number >= $1 AND tx_hash NOT IN (
                SELECT u.tx_hash
                FROM UNNEST ($2::bytea[])
                AS u(tx_hash) 
            )
            ORDER BY sequence_number LIMIT 1000
            "#,
            last_seq_no,
            excluded_tx_hashes
        )
        .fetch_all(self.0.conn())
        .await
        .unwrap();
        let mut last_seq_no = last_seq_no;
        for value in values {
            sqlx::query!(
                "UPDATE tx_filters SET sequence_number = $1, is_priority = true WHERE tx_hash = $2",
                value.sequence_number.unwrap(),
                value.tx_hash
            )
            .execute(self.0.conn())
            .await
            .unwrap();
            last_seq_no = value.sequence_number.unwrap();
        }
        last_seq_no
    }

    // TODO Delete it right after execution
    pub async fn set_seq_no_for_executed_txs(&mut self, last_seq_no: i64) -> i64 {
        let values = sqlx::query!(
            r#"
            SELECT sequence_number, tx_hash 
            FROM executed_transactions where sequence_number >= $1 
            ORDER BY sequence_number 
            LIMIT 1000"#,
            last_seq_no
        )
        .fetch_all(self.0.conn())
        .await
        .unwrap();

        let mut last_seq_no = last_seq_no;
        for value in values {
            sqlx::query!(
                "UPDATE tx_filters SET sequence_number = $1, is_priority=false WHERE tx_hash = $2",
                value.sequence_number.unwrap(),
                &value.tx_hash
            )
            .execute(self.0.conn())
            .await
            .unwrap();
            last_seq_no = value.sequence_number.unwrap();
        }
        last_seq_no
    }

    // TODO Delete it right after execution
    pub async fn get_last_seq_no(&mut self) -> i64 {
        sqlx::query!(
            r#"
            SELECT MAX(sequence_number) AS MAX 
            FROM tx_filters 
            WHERE sequence_number IS NOT NULL
            AND is_priority=false
            "#
        )
        .fetch_one(self.0.conn())
        .await
        .unwrap()
        .max
        .unwrap_or_default()
    }

    // TODO Delete it right after execution
    pub async fn update_non_unique_tx_filters_for_priority_ops(&mut self) -> Vec<Vec<u8>> {
        let mut tx_hash = vec![];
        let mut records = vec![];
        let mut transaction = self.0.start_transaction().await.unwrap();
        sqlx::query!(
            "
            SELECT tx_hash, 
                   to_account, 
                   operation -> 'priority_op' -> 'token' as token_id, 
                   sequence_number 
            FROM executed_priority_operations 
            WHERE tx_hash IN(
                SELECT tx_hash 
                FROM executed_priority_operations 
                GROUP BY (tx_hash) HAVING COUNT(*) > 1
            )
         "
        )
        .fetch_all(transaction.conn())
        .await
        .unwrap()
        .iter()
        .for_each(|a| {
            tx_hash.push(a.tx_hash.as_ref().unwrap().clone());
            records.push((
                a.tx_hash.as_ref().unwrap().clone(),
                a.to_account.as_ref().unwrap().clone(),
                a.token_id.as_ref().unwrap().as_i64().unwrap() as i32,
                a.sequence_number.unwrap(),
            ));
        });
        transaction
            .chain()
            .operations_ext_schema()
            .update_executed_tx_filters(records)
            .await
            .unwrap();
        transaction.commit().await.unwrap();
        tx_hash
    }

    // TODO Delete it right after execution
    pub async fn update_executed_tx_filters(
        &mut self,
        records: Vec<(Vec<u8>, Vec<u8>, i32, i64)>,
    ) -> QueryResult<()> {
        for (tx_hash, address, token, sequence_number) in records {
            sqlx::query!(
                r#"
                UPDATE tx_filters 
                SET sequence_number=$1, is_priority=true 
                WHERE tx_hash = $2 AND address=$3 AND token=$4
                "#,
                sequence_number,
                tx_hash,
                address,
                token
            )
            .execute(self.0.conn())
            .await?;
        }

        Ok(())
    }
}

async fn complete_withdrawals_tx_hash(
    transaction: &mut StorageProcessor<'_>,
    data: &StorageTxData,
) -> QueryResult<Option<H256>> {
    let result = if let Some(tx_type) = data.op.get("type") {
        let tx_type = tx_type.as_str().unwrap();
        if tx_type == "Withdraw" || tx_type == "ForcedExit" {
            transaction
                .chain()
                .operations_schema()
                .eth_tx_for_withdrawal(&TxHash::from_slice(&data.tx_hash).unwrap())
                .await?
        } else {
            None
        }
    } else {
        None
    };
    Ok(result)
}

async fn is_block_finalized(
    transaction: &mut StorageProcessor<'_>,
    block_number: Option<i64>,
) -> QueryResult<Option<bool>> {
    // We always use Option<i64> for block number in cases with this module.
    // So it's much cleaner to keep this check here
    if let Some(block_number) = block_number {
        Ok(Some(
            transaction
                .chain()
                .block_schema()
                .is_block_finalized(BlockNumber(block_number as u32))
                .await?,
        ))
    } else {
        Ok(None)
    }
}

async fn tx_data_from_storage(
    transaction: &mut StorageProcessor<'_>,
    data: StorageTxData,
) -> QueryResult<TxData> {
    let complete_withdrawals_tx_hash = complete_withdrawals_tx_hash(transaction, &data).await?;

    let is_block_finalized = is_block_finalized(transaction, data.block_number).await?;

    Ok(StorageTxData::data_from_storage_data(
        data,
        is_block_finalized,
        complete_withdrawals_tx_hash,
    ))
}
