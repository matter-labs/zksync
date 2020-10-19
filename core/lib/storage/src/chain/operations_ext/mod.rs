// Built-in deps
// External imports
use chrono::{DateTime, Utc};
// Workspace imports
use zksync_types::ActionType;
use zksync_types::{Address, TokenId};
// Local imports
use self::records::{
    AccountCreatedAt, PriorityOpReceiptResponse, TransactionsHistoryItem, TxByHashResponse,
    TxReceiptResponse,
};
use crate::tokens::TokensSchema;
use crate::StorageProcessor;
use crate::{
    chain::operations::{records::StoredExecutedPriorityOperation, OperationsSchema},
    prover::{records::ProverRun, ProverSchema},
    QueryResult,
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
        let tx = OperationsSchema(self.0)
            .get_executed_operation(hash)
            .await?;

        if let Some(tx) = tx {
            // Check whether transaction was verified.
            let verified = OperationsSchema(self.0)
                .get_operation(tx.block_number as u32, ActionType::VERIFY)
                .await
                .map(|v| v.confirmed)
                .unwrap_or(false);

            // Get the prover job details.
            let prover_run = ProverSchema(self.0)
                .get_existing_prover_run(tx.block_number as u32)
                .await?;

            Ok(Some(TxReceiptResponse {
                tx_hash: hex::encode(hash),
                block_number: tx.block_number,
                success: tx.success,
                verified,
                fail_reason: tx.fail_reason,
                prover_run,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_priority_op_receipt(
        &mut self,
        op_id: u32,
    ) -> QueryResult<PriorityOpReceiptResponse> {
        // TODO: jazzandrock maybe use one db query(?).
        let stored_executed_prior_op = OperationsSchema(self.0)
            .get_executed_priority_operation(op_id)
            .await?;

        match stored_executed_prior_op {
            Some(stored_executed_prior_op) => {
                let prover_run: Option<ProverRun> = ProverSchema(self.0)
                    .get_existing_prover_run(stored_executed_prior_op.block_number as u32)
                    .await?;

                let confirm = OperationsSchema(self.0)
                    .get_operation(
                        stored_executed_prior_op.block_number as u32,
                        ActionType::VERIFY,
                    )
                    .await;

                Ok(PriorityOpReceiptResponse {
                    committed: true,
                    verified: confirm.is_some(),
                    prover_run,
                })
            }
            None => Ok(PriorityOpReceiptResponse {
                committed: false,
                verified: false,
                prover_run: None,
            }),
        }
    }

    pub async fn get_tx_by_hash(&mut self, hash: &[u8]) -> QueryResult<Option<TxByHashResponse>> {
        // Attempt to find the transaction in the list of executed operations.
        if let Some(response) = self.find_tx_by_hash(hash).await? {
            return Ok(Some(response));
        }
        // The transaction was not found in the list of executed transactions.
        // Check executed priority operations list.
        if let Some(response) = self.find_priority_op_by_hash(hash).await? {
            return Ok(Some(response));
        }

        // There is no executed transaction with the provided hash.
        Ok(None)
    }

    /// Helper method for `get_tx_by_hash` which attempts to find a transaction
    /// in the list of executed operations.
    async fn find_tx_by_hash(&mut self, hash: &[u8]) -> QueryResult<Option<TxByHashResponse>> {
        // TODO: Maybe move the transformations to api_server?
        let query_result = OperationsSchema(self.0)
            .get_executed_operation(hash)
            .await?;

        if let Some(tx) = query_result {
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

            return Ok(Some(TxByHashResponse {
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
            }));
        };

        Ok(None)
    }

    /// Helper method for `get_tx_by_hash` which attempts to find a transaction
    /// in the list of executed priority operations.
    async fn find_priority_op_by_hash(
        &mut self,
        hash: &[u8],
    ) -> QueryResult<Option<TxByHashResponse>> {
        // TODO: Maybe move the transformations to api_server?
        let tx: Option<StoredExecutedPriorityOperation> = OperationsSchema(self.0)
            .get_executed_priority_operation_by_hash(hash)
            .await?;

        if let Some(tx) = tx {
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

            return Ok(Some(TxByHashResponse {
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
            }));
        };

        Ok(None)
    }

    /// Loads the date and time of the moment when the first transaction for the account was executed.
    /// Can be `None` if there were no transactions associated with provided address.
    pub async fn account_created_on(
        &mut self,
        address: &Address,
    ) -> QueryResult<Option<DateTime<Utc>>> {
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
        // let query = format!(
        //     ,
        //     address = hex::encode(address.as_ref().to_vec()),
        //     offset = offset,
        //     limit = limit
        // );
        let mut tx_history = sqlx::query_as!(
            TransactionsHistoryItem,
            r#"
            with eth_ops as (
                select distinct on (block_number, action_type)
                    operations.block_number,
                    operations.action_type,
                    confirmed
                from operations
                order by block_number desc, action_type, confirmed
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
            left join eth_ops verified on
                verified.block_number = transactions.block_number and verified.action_type = 'VERIFY' and verified.confirmed = true
            order by transactions.block_number desc, created_at desc
            "#,
            address.as_ref(), offset as i64, limit as i64
        ).fetch_all(self.0.conn())
        .await?;

        // diesel::sql_query(query).load::<TransactionsHistoryItem>(self.0.conn())?;
        if !tx_history.is_empty() {
            let tokens = TokensSchema(self.0).load_tokens().await?;
            for tx_item in &mut tx_history {
                let tx_info = match tx_item.tx["type"].as_str().unwrap_or("NONE") {
                    "NONE" => {
                        log::warn!("Tx history item type not found, tx: {:?}", tx_item);
                        continue;
                    }
                    "Deposit" | "FullExit" => tx_item.tx.get_mut("priority_op"),
                    _ => Some(&mut tx_item.tx),
                };

                let tx_info = if let Some(tx_info) = tx_info {
                    tx_info
                } else {
                    log::warn!("tx_info not found for tx: {:?}", tx_item);
                    continue;
                };

                if let Some(tok_val) = tx_info.get_mut("token") {
                    if let Some(token_id) = tok_val.as_u64() {
                        let token_id = token_id as TokenId;
                        let token_symbol = tokens
                            .get(&token_id)
                            .map(|t| t.symbol.clone())
                            .unwrap_or_else(|| "UNKNOWN".to_string());
                        *tok_val =
                            serde_json::to_value(token_symbol).expect("json string to value");
                    };
                };
            }
        }
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
        // Filter for txs that older/newer than provided tx ID.
        // For older blocks, block number should be between 0 and block number - 1,
        // or for the same block number, transaction in block should be between 0 and tx in block number - 1.
        // For newer filter range starts on the ID + 1 and ends in the max value for the type correspondingly.
        let (block_id, block_tx_id) = tx_id;
        let (block_number_start_idx, block_number_end_idx) = match direction {
            SearchDirection::Older => (0i64, block_id as i64 - 1), // Older blocks have lesser block ID.
            SearchDirection::Newer => (block_id as i64 + 1, i64::max_value()), // Newer blocks have greater block ID.
        };
        let (tx_number_start_idx, tx_number_end_idx) = match direction {
            SearchDirection::Older => (0i32, block_tx_id as i32 - 1),
            SearchDirection::Newer => (block_tx_id as i32 + 1, i32::max_value()),
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
            with eth_ops as (
                select distinct on (block_number, action_type)
                    operations.block_number,
                    operations.action_type,
                    confirmed
                from operations
                order by block_number desc, action_type, confirmed
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
            left join eth_ops committed on
                committed.block_number = transactions.block_number and committed.action_type = 'COMMIT' and committed.confirmed = true
            left join eth_ops verified on
                verified.block_number = transactions.block_number and verified.action_type = 'VERIFY' and verified.confirmed = true
            order by transactions.block_number desc, created_at desc
            "#,
            address.as_ref(),
            block_id as i64,
            block_number_start_idx, block_number_end_idx,
            tx_number_start_idx, tx_number_end_idx,
            limit as i64
        ).fetch_all(self.0.conn())
        .await?;

        // diesel::sql_query(query).load::<TransactionsHistoryItem>(self.0.conn())?;
        if !tx_history.is_empty() {
            let tokens = TokensSchema(self.0).load_tokens().await?;
            for tx_item in &mut tx_history {
                let tx_info = match tx_item.tx["type"].as_str().unwrap_or("NONE") {
                    "NONE" => {
                        log::warn!("Tx history item type not found, tx: {:?}", tx_item);
                        continue;
                    }
                    "Deposit" | "FullExit" => tx_item.tx.get_mut("priority_op"),
                    _ => Some(&mut tx_item.tx),
                };

                let tx_info = if let Some(tx_info) = tx_info {
                    tx_info
                } else {
                    log::warn!("tx_info not found for tx: {:?}", tx_item);
                    continue;
                };

                if let Some(tok_val) = tx_info.get_mut("token") {
                    if let Some(token_id) = tok_val.as_u64() {
                        let token_id = token_id as TokenId;
                        let token_symbol = tokens
                            .get(&token_id)
                            .map(|t| t.symbol.clone())
                            .unwrap_or_else(|| "UNKNOWN".to_string());
                        *tok_val =
                            serde_json::to_value(token_symbol).expect("json string to value");
                    };
                };
            }
        }
        Ok(tx_history)
    }
}
