// Built-in deps
use std::time::Instant;

// External imports
use chrono::{DateTime, Utc};

// Workspace imports
use zksync_crypto::params;
use zksync_types::aggregated_operations::AggregatedActionType;
use zksync_types::{Address, BlockNumber, TokenId};

// Local imports
use self::records::{
    AccountCreatedAt, AccountOpReceiptResponse, AccountTxReceiptResponse,
    PriorityOpReceiptResponse, TransactionsHistoryItem, TxByHashResponse, TxReceiptResponse,
};
use crate::{
    chain::operations::{records::StoredExecutedPriorityOperation, OperationsSchema},
    tokens::TokensSchema,
    QueryResult, StorageProcessor,
};

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
            .get_executed_priority_operation_by_hash(hash)
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

    /// Loads the range of transaction receipts applied to the given account address
    /// starting from the specified transaction location. Transaction location is defined
    /// by the (`block_number`, `block index`) pair. This method can be used to get receipts
    /// "older" than some location or "newer" than one.
    ///
    /// The response for "newer" receipts is sorted in ascending order by position and for "older"
    /// ones in descending order.
    pub async fn get_account_transactions_receipts(
        &mut self,
        address: Address,
        block_number: u64,
        block_index: Option<u32>,
        direction: SearchDirection,
        limit: u64,
    ) -> QueryResult<Vec<AccountTxReceiptResponse>> {
        let start = Instant::now();

        let block_number = block_number as i64;
        let block_index = block_index.map(|x| x as i32).unwrap_or(-1);

        let receipts: Vec<_> = match direction {
            SearchDirection::Newer => {
                sqlx::query_as!(
                    AccountTxReceiptResponse,
                    r#"
                    WITH block_details AS (
                        WITH aggr_comm AS (
                            SELECT 
                                aggregate_operations.created_at, 
                                eth_operations.final_hash, 
                                commit_aggregated_blocks_binding.block_number 
                            FROM aggregate_operations
                                INNER JOIN commit_aggregated_blocks_binding ON aggregate_operations.id = commit_aggregated_blocks_binding.op_id
                                INNER JOIN eth_aggregated_ops_binding ON aggregate_operations.id = eth_aggregated_ops_binding.op_id
                                INNER JOIN eth_operations ON eth_operations.id = eth_aggregated_ops_binding.eth_op_id
                            WHERE aggregate_operations.confirmed = true 
                        )
                        , aggr_exec as (
                             SELECT 
                                aggregate_operations.created_at, 
                                eth_operations.final_hash, 
                                execute_aggregated_blocks_binding.block_number 
                            FROM aggregate_operations
                                INNER JOIN execute_aggregated_blocks_binding ON aggregate_operations.id = execute_aggregated_blocks_binding.op_id
                                INNER JOIN eth_aggregated_ops_binding ON aggregate_operations.id = eth_aggregated_ops_binding.op_id
                                INNER JOIN eth_operations ON eth_operations.id = eth_aggregated_ops_binding.eth_op_id
                            WHERE aggregate_operations.confirmed = true 
                        )
                        SELECT
                            blocks.number AS details_block_number,
                            committed.final_hash AS commit_tx_hash,
                            verified.final_hash AS verify_tx_hash
                        FROM blocks
                                INNER JOIN aggr_comm committed ON blocks.number = committed.block_number
                                LEFT JOIN aggr_exec verified ON blocks.number = verified.block_number
                        )
                    SELECT
                        block_number, 
                        block_index as "block_index?",
                        tx_hash,
                        success,
                        fail_reason as "fail_reason?",
                        details.commit_tx_hash as "commit_tx_hash?",
                        details.verify_tx_hash as "verify_tx_hash?"
                    FROM executed_transactions
                    LEFT JOIN block_details details ON details.details_block_number = executed_transactions.block_number
                    WHERE (
                        (primary_account_address = $1 OR from_account = $1 OR to_account = $1)
                        AND (
                            block_number = $2 AND (
                                COALESCE(block_index, -1) >= $3
                            ) OR (
                                block_number > $2
                            )
                        )
                    )
                    ORDER BY block_number ASC, COALESCE(block_index, -1) ASC
                    LIMIT $4
                    "#,
                    address.as_bytes(),
                    block_number,
                    block_index,
                    limit as i64,
                ).fetch_all(self.0.conn())
                .await?
            },

            SearchDirection::Older => {
                sqlx::query_as!(
                    AccountTxReceiptResponse,
                    r#"
                    WITH block_details AS (
                        WITH aggr_comm AS (
                            SELECT 
                                aggregate_operations.created_at, 
                                eth_operations.final_hash, 
                                commit_aggregated_blocks_binding.block_number 
                            FROM aggregate_operations
                                INNER JOIN commit_aggregated_blocks_binding ON aggregate_operations.id = commit_aggregated_blocks_binding.op_id
                                INNER JOIN eth_aggregated_ops_binding ON aggregate_operations.id = eth_aggregated_ops_binding.op_id
                                INNER JOIN eth_operations ON eth_operations.id = eth_aggregated_ops_binding.eth_op_id
                            WHERE aggregate_operations.confirmed = true 
                        )
                        , aggr_exec as (
                             SELECT 
                                aggregate_operations.created_at, 
                                eth_operations.final_hash, 
                                execute_aggregated_blocks_binding.block_number 
                            FROM aggregate_operations
                                INNER JOIN execute_aggregated_blocks_binding ON aggregate_operations.id = execute_aggregated_blocks_binding.op_id
                                INNER JOIN eth_aggregated_ops_binding ON aggregate_operations.id = eth_aggregated_ops_binding.op_id
                                INNER JOIN eth_operations ON eth_operations.id = eth_aggregated_ops_binding.eth_op_id
                            WHERE aggregate_operations.confirmed = true 
                        )
                        SELECT
                            blocks.number AS details_block_number,
                            committed.final_hash AS commit_tx_hash,
                            verified.final_hash AS verify_tx_hash
                        FROM blocks
                                INNER JOIN aggr_comm committed ON blocks.number = committed.block_number
                                LEFT JOIN aggr_exec verified ON blocks.number = verified.block_number
                    )
                    SELECT
                        block_number, 
                        block_index as "block_index?",
                        tx_hash,
                        success,
                        fail_reason as "fail_reason?",
                        details.commit_tx_hash as "commit_tx_hash?",
                        details.verify_tx_hash as "verify_tx_hash?"
                    FROM executed_transactions
                    LEFT JOIN block_details details ON details.details_block_number = executed_transactions.block_number
                    WHERE (
                        (primary_account_address = $1 OR from_account = $1 OR to_account = $1)
                        AND (
                            block_number = $2 AND (
                                COALESCE(block_index, -1) <= $3
                            ) OR (
                                block_number < $2
                            )
                        )
                    )
                    ORDER BY block_number DESC, COALESCE(block_index, -1) DESC
                    LIMIT $4
                    "#,
                    address.as_bytes(),
                    block_number,
                    block_index,
                    limit as i64,
                ).fetch_all(self.0.conn())
                .await?
            }
        };

        metrics::histogram!(
            "sql.chain.operations_ext.get_account_transactions_receipts",
            start.elapsed()
        );
        Ok(receipts)
    }

    /// Loads the range of priority operation receipts applied to the given account address
    /// starting from the specified operation location. Transaction location is defined
    /// by the (`block_number`, `block index`) pair. This method can be used to get receipts
    /// "older" than some location or "newer" than one.
    ///
    /// The response for "newer" receipts is sorted in ascending order by position and for "older"
    /// ones in descending order.
    pub async fn get_account_operations_receipts(
        &mut self,
        address: Address,
        block_number: u64,
        block_index: u32,
        direction: SearchDirection,
        limit: u64,
    ) -> QueryResult<Vec<AccountOpReceiptResponse>> {
        let start = Instant::now();

        let block_number = block_number as i64;
        let block_index = block_index as i32;

        let receipts: Vec<_> = match direction {
            SearchDirection::Newer => {
                sqlx::query_as!(
                    AccountOpReceiptResponse,
                    r#"
                    WITH block_details AS (
                        WITH aggr_comm AS (
                            SELECT 
                                aggregate_operations.created_at, 
                                eth_operations.final_hash, 
                                commit_aggregated_blocks_binding.block_number 
                            FROM aggregate_operations
                                INNER JOIN commit_aggregated_blocks_binding ON aggregate_operations.id = commit_aggregated_blocks_binding.op_id
                                INNER JOIN eth_aggregated_ops_binding ON aggregate_operations.id = eth_aggregated_ops_binding.op_id
                                INNER JOIN eth_operations ON eth_operations.id = eth_aggregated_ops_binding.eth_op_id
                            WHERE aggregate_operations.confirmed = true 
                        )
                        , aggr_exec as (
                             SELECT 
                                aggregate_operations.created_at, 
                                eth_operations.final_hash, 
                                execute_aggregated_blocks_binding.block_number 
                            FROM aggregate_operations
                                INNER JOIN execute_aggregated_blocks_binding ON aggregate_operations.id = execute_aggregated_blocks_binding.op_id
                                INNER JOIN eth_aggregated_ops_binding ON aggregate_operations.id = eth_aggregated_ops_binding.op_id
                                INNER JOIN eth_operations ON eth_operations.id = eth_aggregated_ops_binding.eth_op_id
                            WHERE aggregate_operations.confirmed = true 
                        )
                        SELECT
                            blocks.number AS details_block_number,
                            committed.final_hash AS commit_tx_hash,
                            verified.final_hash AS verify_tx_hash
                        FROM blocks
                                INNER JOIN aggr_comm committed ON blocks.number = committed.block_number
                                LEFT JOIN aggr_exec verified ON blocks.number = verified.block_number
                    )
                    SELECT
                        block_number, 
                        block_index,
                        eth_hash,
                        details.commit_tx_hash as "commit_tx_hash?",
                        details.verify_tx_hash as "verify_tx_hash?"
                    FROM executed_priority_operations
                    LEFT JOIN block_details details ON details.details_block_number = executed_priority_operations.block_number
                    WHERE (
                        (from_account = $1 OR to_account = $1)
                        AND (
                            block_number = $2 AND (
                                block_index >= $3
                            ) OR (
                                block_number > $2
                            )
                        )
                    )
                    ORDER BY block_number ASC, block_index ASC
                    LIMIT $4
                    "#,
                    address.as_bytes(),
                    block_number,
                    block_index,
                    limit as i64,
                ).fetch_all(self.0.conn())
                .await?
            },

            SearchDirection::Older => {
                sqlx::query_as!(
                    AccountOpReceiptResponse,
                    r#"
                    WITH block_details AS (
                        WITH aggr_comm AS (
                            SELECT 
                                aggregate_operations.created_at, 
                                eth_operations.final_hash, 
                                commit_aggregated_blocks_binding.block_number 
                            FROM aggregate_operations
                                INNER JOIN commit_aggregated_blocks_binding ON aggregate_operations.id = commit_aggregated_blocks_binding.op_id
                                INNER JOIN eth_aggregated_ops_binding ON aggregate_operations.id = eth_aggregated_ops_binding.op_id
                                INNER JOIN eth_operations ON eth_operations.id = eth_aggregated_ops_binding.eth_op_id
                            WHERE aggregate_operations.confirmed = true 
                        )
                        , aggr_exec as (
                             SELECT 
                                aggregate_operations.created_at, 
                                eth_operations.final_hash, 
                                execute_aggregated_blocks_binding.block_number 
                            FROM aggregate_operations
                                INNER JOIN execute_aggregated_blocks_binding ON aggregate_operations.id = execute_aggregated_blocks_binding.op_id
                                INNER JOIN eth_aggregated_ops_binding ON aggregate_operations.id = eth_aggregated_ops_binding.op_id
                                INNER JOIN eth_operations ON eth_operations.id = eth_aggregated_ops_binding.eth_op_id
                            WHERE aggregate_operations.confirmed = true 
                        )
                        SELECT
                            blocks.number AS details_block_number,
                            committed.final_hash AS commit_tx_hash,
                            verified.final_hash AS verify_tx_hash
                        FROM blocks
                                INNER JOIN aggr_comm committed ON blocks.number = committed.block_number
                                LEFT JOIN aggr_exec verified ON blocks.number = verified.block_number
                    )
                    SELECT
                        block_number, 
                        block_index,
                        eth_hash,
                        details.commit_tx_hash as "commit_tx_hash?",
                        details.verify_tx_hash as "verify_tx_hash?"
                    FROM executed_priority_operations
                    LEFT JOIN block_details details ON details.details_block_number = executed_priority_operations.block_number
                    WHERE (
                        (from_account = $1 OR to_account = $1)
                        AND (
                            block_number = $2 AND (
                                block_index <= $3
                            ) OR (
                                block_number < $2
                            )
                        )
                    )
                    ORDER BY block_number DESC, block_index DESC
                    LIMIT $4
                    "#,
                    address.as_bytes(),
                    block_number,
                    block_index,
                    limit as i64,
                ).fetch_all(self.0.conn())
                .await?
            }
        };

        metrics::histogram!(
            "sql.chain.operations_ext.get_account_operations_receipts",
            start.elapsed()
        );
        Ok(receipts)
    }
}
