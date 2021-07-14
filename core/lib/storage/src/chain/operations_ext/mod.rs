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
    aggregated_operations::AggregatedActionType,
    {tx::TxHash, Address, BlockNumber, TokenId},
};

// Local imports
use self::records::{
    AccountCreatedAt, InBlockBatchTx, PriorityOpReceiptResponse, StorageTxData, StorageTxReceipt,
    TransactionsHistoryItem, TxByHashResponse, TxReceiptResponse,
};
use crate::{
    chain::{
        block::records::TransactionItem,
        operations::{records::StoredExecutedPriorityOperation, OperationsSchema},
    },
    tokens::TokensSchema,
    QueryResult, StorageProcessor,
};

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
            let is_block_finalized = if let Some(block_number) = receipt.block_number {
                Some(
                    transaction
                        .chain()
                        .block_schema()
                        .is_block_finalized(BlockNumber(block_number as u32))
                        .await?,
                )
            } else {
                None
            };
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
                        created_at,
                        success,
                        fail_reason,
                        Null::bytea as eth_hash,
                        Null::bigint as priority_op_serialid,
                        eth_sign_data
                    FROM executed_transactions
                    WHERE tx_hash = $1
                ), priority_op AS (
                    SELECT
                        tx_hash,
                        operation as op,
                        block_number,
                        created_at,
                        true as success,
                        Null as fail_reason,
                        eth_hash,
                        priority_op_serialid,
                        Null::jsonb as eth_sign_data
                    FROM executed_priority_operations
                    WHERE tx_hash = $1 OR eth_hash = $1
                ), mempool_tx AS (
                    SELECT
                        decode(tx_hash, 'hex'),
                        tx as op,
                        Null::bigint as block_number,
                        created_at,
                        Null::boolean as success,
                        Null as fail_reason,
                        Null::bytea as eth_hash,
                        Null::bigint as priority_op_serialid,
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
                    created_at as "created_at!",
                    success as "success?",
                    fail_reason as "fail_reason?",
                    eth_hash as "eth_hash?",
                    priority_op_serialid as "priority_op_serialid?",
                    eth_sign_data as "eth_sign_data?"
                FROM everything
            "#,
            hash,
            &hash_str
        )
        .fetch_optional(transaction.conn())
        .await?;

        let result = if let Some(data) = data {
            let complete_withdrawals_tx_hash = if let Some(tx_type) = data.op.get("type") {
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

            let is_block_finalized = if let Some(block_number) = data.block_number {
                Some(
                    transaction
                        .chain()
                        .block_schema()
                        .is_block_finalized(BlockNumber(block_number as u32))
                        .await?,
                )
            } else {
                None
            };

            Some(StorageTxData::data_from_storage_data(
                data,
                is_block_finalized,
                complete_withdrawals_tx_hash,
            ))
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
            ),
            transactions AS (
                SELECT
                    *
                FROM (
                    SELECT
                        concat_ws(',', block_number, block_index) AS tx_id,
                        tx,
                        'sync-tx:' || encode(tx_hash, 'hex') AS hash,
                        null as pq_id,
                        null as eth_block,
                        success,
                        fail_reason,
                        block_number,
                        created_at
                    FROM
                        executed_transactions
                    WHERE
                        from_account = $1
                        or
                        to_account = $1
                        or
                        primary_account_address = $1
                    union all
                    select
                        concat_ws(',', block_number, block_index) as tx_id,
                        operation as tx,
                        '0x' || encode(eth_hash, 'hex') as hash,
                        priority_op_serialid as pq_id,
                        eth_block,
                        true as success,
                        null as fail_reason,
                        block_number,
                        created_at
                    from 
                        executed_priority_operations
                    where 
                        from_account = $1
                        or
                        to_account = $1) t
                order by
                    block_number desc, created_at desc
                offset 
                    $2
                limit 
                    $3
            )
            select
                tx_id as "tx_id!",
                hash as "hash?",
                eth_block as "eth_block?",
                pq_id as "pq_id?",
                tx as "tx!",
                success as "success?",
                fail_reason as "fail_reason?",
                true as "commited!",
                coalesce(verified.confirmed, false) as "verified!",
                created_at as "created_at!"
            from transactions
            LEFT JOIN aggr_exec verified ON transactions.block_number = verified.block_number
            order by transactions.block_number desc, created_at desc
            "#,
            address.as_ref(), offset as i64, limit as i64
        ).fetch_all(self.0.conn())
        .await?;

        if !tx_history.is_empty() {
            let tokens = TokensSchema(self.0).load_tokens().await?;
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

        metrics::histogram!(
            "sql.chain.operations_ext.get_account_transactions_history",
            start.elapsed()
        );
        Ok(tx_history)
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
        // Filter for txs that older/newer than provided tx ID.
        // For older blocks, block number should be between 0 and block number - 1,
        // or for the same block number, transaction in block should be between 0 and tx in block number - 1.
        // For newer filter range starts on the ID + 1 and ends in the max value for the type correspondingly.
        let (block_id, block_tx_id) = tx_id;
        let (block_number_start_idx, block_number_end_idx) = match direction {
            SearchDirection::Older => (0i64, block_id as i64 - 1), // Older blocks have lesser block ID.
            SearchDirection::Newer => (block_id as i64 + 1, i64::MAX), // Newer blocks have greater block ID.
        };
        let (tx_number_start_idx, tx_number_end_idx) = match direction {
            SearchDirection::Older => (0i32, block_tx_id as i32 - 1),
            SearchDirection::Newer => (block_tx_id as i32 + 1, i32::MAX),
        };

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
            ), transactions as (
                select
                    *
                from (
                    select
                        concat_ws(',', block_number, block_index) as tx_id,
                        tx,
                        'sync-tx:' || encode(tx_hash, 'hex') as hash,
                        null as pq_id,
                        null as eth_block,
                        success,
                        fail_reason,
                        block_number,
                        created_at
                    from
                        executed_transactions
                    where
                        (
                            from_account = $1
                            or
                            to_account = $1
                            or
                            primary_account_address = $1
                        )
                        and
                        (block_number BETWEEN $3 AND $4 or (block_number = $2 and block_index BETWEEN $5 AND $6))
                    union all
                    select
                        concat_ws(',', block_number, block_index) as tx_id,
                        operation as tx,
                        '0x' || encode(eth_hash, 'hex') as hash,
                        priority_op_serialid as pq_id,
                        eth_block,
                        true as success,
                        null as fail_reason,
                        block_number,
                        created_at
                    from 
                        executed_priority_operations
                    where 
                        (
                            from_account = $1
                            or
                            to_account = $1
                        )
                        and
                        (block_number BETWEEN $3 AND $4 or (block_number = $2 and block_index BETWEEN $5 AND $6))
                    ) t
                order by
                    block_number desc, created_at desc
                limit 
                    $7
            )
            select
                tx_id as "tx_id!",
                hash as "hash?",
                eth_block as "eth_block?",
                pq_id as "pq_id?",
                tx as "tx!",
                success as "success?",
                fail_reason as "fail_reason?",
                true as "commited!",
                coalesce(verified.confirmed, false) as "verified!",
                created_at as "created_at!"
            from transactions
            left join aggr_comm committed on
                committed.block_number = transactions.block_number AND committed.confirmed = true
            left join aggr_exec verified on
                verified.block_number = transactions.block_number AND verified.confirmed = true
            order by transactions.block_number desc, created_at desc
            "#,
            address.as_ref(),
            block_id as i64,
            block_number_start_idx, block_number_end_idx,
            tx_number_start_idx, tx_number_end_idx,
            limit as i64
        ).fetch_all(self.0.conn())
        .await?;

        if !tx_history.is_empty() {
            let tokens = TokensSchema(self.0).load_tokens().await?;
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
        let created_at_and_block = transaction
            .chain()
            .operations_ext_schema()
            .get_tx_created_at_and_block_number(tx_hash)
            .await?;
        let txs = if let Some((time_from, _)) = created_at_and_block {
            let raw_txs: Vec<TransactionItem> = match query.direction {
                PaginationDirection::Newer => {
                    sqlx::query_as!(
                        TransactionItem,
                        r#"
                            WITH transactions AS (
                                SELECT
                                    tx_hash,
                                    tx as op,
                                    block_number,
                                    created_at,
                                    success,
                                    fail_reason,
                                    Null::bytea as eth_hash,
                                    Null::bigint as priority_op_serialid,
                                    block_index
                                FROM executed_transactions
                                WHERE (from_account = $1 OR to_account = $1 OR primary_account_address = $1)
                                    AND created_at >= $2
                                ), priority_ops AS (
                                SELECT
                                    tx_hash,
                                    operation as op,
                                    block_number,
                                    created_at,
                                    true as success,
                                    Null as fail_reason,
                                    eth_hash,
                                    priority_op_serialid,
                                    block_index
                                FROM executed_priority_operations
                                WHERE (from_account = $1 OR to_account = $1) AND created_at >= $2
                            ), everything AS (
                                SELECT * FROM transactions
                                UNION ALL
                                SELECT * FROM priority_ops
                            )
                            SELECT
                                tx_hash as "tx_hash!",
                                block_number as "block_number!",
                                op as "op!",
                                created_at as "created_at!",
                                success as "success!",
                                fail_reason as "fail_reason?",
                                eth_hash as "eth_hash?",
                                priority_op_serialid as "priority_op_serialid?"
                            FROM everything
                            ORDER BY created_at ASC, block_index ASC
                            LIMIT $3
                        "#,
                        query.from.address.as_bytes(),
                        time_from,
                        i64::from(query.limit),
                    )
                    .fetch_all(transaction.conn())
                    .await?
                }
                PaginationDirection::Older => {
                    sqlx::query_as!(
                        TransactionItem,
                        r#"
                            WITH transactions AS (
                                SELECT
                                    tx_hash,
                                    tx as op,
                                    block_number,
                                    created_at,
                                    success,
                                    fail_reason,
                                    Null::bytea as eth_hash,
                                    Null::bigint as priority_op_serialid,
                                    block_index
                                FROM executed_transactions
                                WHERE (from_account = $1 OR to_account = $1 OR primary_account_address = $1)
                                    AND created_at <= $2
                            ), priority_ops AS (
                                SELECT
                                    tx_hash,
                                    operation as op,
                                    block_number,
                                    created_at,
                                    true as success,
                                    Null as fail_reason,
                                    eth_hash,
                                    priority_op_serialid,
                                    block_index
                                FROM executed_priority_operations
                                WHERE (from_account = $1 OR to_account = $1) AND created_at <= $2
                            ), everything AS (
                                SELECT * FROM transactions
                                UNION ALL
                                SELECT * FROM priority_ops
                            )
                            SELECT
                                tx_hash as "tx_hash!",
                                block_number as "block_number!",
                                op as "op!",
                                created_at as "created_at!",
                                success as "success!",
                                fail_reason as "fail_reason?",
                                eth_hash as "eth_hash?",
                                priority_op_serialid as "priority_op_serialid?"
                            FROM everything
                            ORDER BY created_at DESC, block_index DESC
                            LIMIT $3
                        "#,
                        query.from.address.as_bytes(),
                        time_from,
                        i64::from(query.limit),
                    )
                    .fetch_all(transaction.conn())
                    .await?
                }
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

    pub async fn get_account_last_tx_hash(
        &mut self,
        address: Address,
    ) -> QueryResult<Option<TxHash>> {
        let start = Instant::now();
        let record = sqlx::query!(
            r#"
                WITH transactions AS (
                    SELECT tx_hash, created_at, block_index
                    FROM executed_transactions
                    WHERE from_account = $1 OR to_account = $1 OR primary_account_address = $1
                ), priority_ops AS (
                    SELECT tx_hash, created_at, block_index
                    FROM executed_priority_operations
                    WHERE from_account = $1 OR to_account = $1
                ), everything AS (
                    SELECT * FROM transactions
                    UNION ALL
                    SELECT * FROM priority_ops
                )
                SELECT
                    tx_hash as "tx_hash!"
                FROM everything
                ORDER BY created_at DESC, block_index DESC
                LIMIT 1
            "#,
            address.as_bytes(),
        )
        .fetch_optional(self.0.conn())
        .await?;

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
                    SELECT tx_hash, created_at, block_index
                    FROM executed_transactions
                    WHERE block_number = $1
                ), priority_ops AS (
                    SELECT tx_hash, created_at, block_index
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
                ORDER BY created_at DESC, block_index DESC
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

    pub async fn get_account_transactions_count(&mut self, address: Address) -> QueryResult<u32> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;
        let last_committed = transaction
            .chain()
            .block_schema()
            .get_last_committed_confirmed_block()
            .await?;
        let tx_count = sqlx::query!(
            r#"
                SELECT COUNT(*) as "count!" FROM executed_transactions
                WHERE block_number <= $1 AND (from_account = $2 OR to_account = $2 OR primary_account_address = $2)
            "#,
            i64::from(*last_committed),
            address.as_bytes()
        )
        .fetch_one(transaction.conn())
        .await?
        .count;

        let priority_op_count = sqlx::query!(
            r#"
                SELECT COUNT(*) as "count!" FROM executed_priority_operations
                WHERE block_number <= $1 AND (from_account = $2 OR to_account = $2)
            "#,
            i64::from(*last_committed),
            address.as_bytes()
        )
        .fetch_one(transaction.conn())
        .await?
        .count;
        transaction.commit().await?;

        metrics::histogram!(
            "sql.chain.operations_ext.get_account_transactions_count",
            start.elapsed()
        );
        Ok((tx_count + priority_op_count) as u32)
    }

    /// Returns `created_at` and `block_number` fields for transaction with given hash.
    pub async fn get_tx_created_at_and_block_number(
        &mut self,
        tx_hash: TxHash,
    ) -> QueryResult<Option<(DateTime<Utc>, BlockNumber)>> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let record = sqlx::query!(
            "SELECT created_at, block_number FROM executed_transactions
            WHERE tx_hash = $1",
            tx_hash.as_ref()
        )
        .fetch_optional(transaction.conn())
        .await?;

        let result = if let Some(record) = record {
            Some((record.created_at, BlockNumber(record.block_number as u32)))
        } else {
            let record = sqlx::query!(
                "SELECT created_at, block_number FROM executed_priority_operations
                WHERE tx_hash = $1",
                tx_hash.as_ref()
            )
            .fetch_optional(transaction.conn())
            .await?;

            record.map(|record| (record.created_at, BlockNumber(record.block_number as u32)))
        };
        transaction.commit().await?;

        metrics::histogram!(
            "sql.chain.block.get_tx_created_at_and_block_number",
            start.elapsed()
        );
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
                ORDER BY created_at ASC, block_index ASC
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
}
