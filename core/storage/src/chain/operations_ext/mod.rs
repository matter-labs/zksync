// Built-in deps
// External imports
use chrono::{DateTime, Utc};
use diesel::prelude::*;
// Workspace imports
use models::node::{Address, TokenId};
use models::ActionType;
// Local imports
use self::records::{
    AccountCreatedAt, PriorityOpReceiptResponse, TransactionsHistoryItem, TxByHashResponse,
    TxReceiptResponse,
};
use crate::schema::*;
use crate::tokens::TokensSchema;
use crate::StorageProcessor;
use crate::{
    chain::operations::{
        records::{StoredExecutedPriorityOperation, StoredExecutedTransaction, StoredOperation},
        OperationsSchema,
    },
    prover::records::ProverRun,
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
pub struct OperationsExtSchema<'a>(pub &'a StorageProcessor);

impl<'a> OperationsExtSchema<'a> {
    pub fn tx_receipt(&self, hash: &[u8]) -> QueryResult<Option<TxReceiptResponse>> {
        let tx = OperationsSchema(self.0).get_executed_operation(hash)?;

        if let Some(tx) = tx {
            // Check whether transaction was verified.
            let verified = operations::table
                .filter(operations::block_number.eq(tx.block_number))
                .filter(operations::action_type.eq(ActionType::VERIFY.to_string()))
                .first::<StoredOperation>(self.0.conn())
                .optional()?
                .map(|v| v.confirmed)
                .unwrap_or(false);

            // Get the prover job details.
            let prover_run: Option<ProverRun> = prover_runs::table
                .filter(prover_runs::block_number.eq(tx.block_number))
                .first::<ProverRun>(self.0.conn())
                .optional()?;

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

    pub fn get_priority_op_receipt(&self, op_id: u32) -> QueryResult<PriorityOpReceiptResponse> {
        // TODO: jazzandrock maybe use one db query(?).
        let stored_executed_prior_op =
            OperationsSchema(self.0).get_executed_priority_operation(op_id)?;

        match stored_executed_prior_op {
            Some(stored_executed_prior_op) => {
                let prover_run: Option<ProverRun> = prover_runs::table
                    .filter(prover_runs::block_number.eq(stored_executed_prior_op.block_number))
                    .first::<ProverRun>(self.0.conn())
                    .optional()?;

                let confirm = operations::table
                    .filter(operations::block_number.eq(stored_executed_prior_op.block_number))
                    .filter(operations::action_type.eq(ActionType::VERIFY.to_string()))
                    .first::<StoredOperation>(self.0.conn())
                    .optional()?;

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

    pub fn get_tx_by_hash(&self, hash: &[u8]) -> QueryResult<Option<TxByHashResponse>> {
        // Attempt to find the transaction in the list of executed operations.
        if let Some(response) = self.find_tx_by_hash(hash)? {
            return Ok(Some(response));
        }
        // The transaction was not found in the list of executed transactions.
        // Check executed priority operations list.
        if let Some(response) = self.find_priority_op_by_hash(hash)? {
            return Ok(Some(response));
        }

        // There is no executed transaction with the provided hash.
        Ok(None)
    }

    /// Helper method for `get_tx_by_hash` which attempts to find a transaction
    /// in the list of executed operations.
    fn find_tx_by_hash(&self, hash: &[u8]) -> QueryResult<Option<TxByHashResponse>> {
        // TODO: Maybe move the transformations to api_server?
        let query_result = executed_transactions::table
            .filter(executed_transactions::tx_hash.eq(hash))
            .first::<StoredExecutedTransaction>(self.0.conn())
            .optional()?;

        if let Some(tx) = query_result {
            let block_number = tx.block_number;
            let fail_reason = tx.fail_reason.clone();
            let created_at = tx.created_at.format("%Y-%m-%dT%H:%M:%S%.6f").to_string();
            let operation = &tx.tx;

            let tx_token = operation["token"].as_i64().unwrap_or(-1);
            let tx_type = operation["type"].as_str().unwrap_or("unknown tx_type");
            let tx_amount = operation["amount"].as_str().unwrap_or("unknown amount");
            let nonce = operation["nonce"].as_i64().unwrap_or(-1);
            let (tx_from, tx_to, tx_fee) = match tx_type {
                "Withdraw" | "Transfer" | "TransferToNew" => (
                    operation["from"]
                        .as_str()
                        .unwrap_or("unknown from")
                        .to_string(),
                    operation["to"].as_str().unwrap_or("unknown to").to_string(),
                    operation["fee"].as_str().map(|v| v.to_string()),
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
                    None,
                ),
                &_ => (
                    "unknown from".to_string(),
                    "unknown to".to_string(),
                    Some("unknown fee".to_string()),
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
                amount: tx_amount.to_string(),
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
    fn find_priority_op_by_hash(&self, hash: &[u8]) -> QueryResult<Option<TxByHashResponse>> {
        // TODO: Maybe move the transformations to api_server?
        let tx: Option<StoredExecutedPriorityOperation> = executed_priority_operations::table
            .filter(executed_priority_operations::eth_hash.eq(hash))
            .first(self.0.conn())
            .optional()?;

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
    pub fn account_created_on(&self, address: &Address) -> QueryResult<Option<DateTime<Utc>>> {
        // This query loads the `committed_at` field from both `executed_transactions` and
        // `executed_priority_operations` tables and returns the oldest result.
        let query = format!(
            "
            select 
                created_at
            from (
                    with vars (address_bytes) as ( select decode('{address}', 'hex') )
                    select
                        created_at
                    from
                        executed_transactions, vars
                    where
                        from_account = address_bytes
                        or
                        to_account = address_bytes
                        or
                        primary_account_address = address_bytes
                    union all
                    select
                        created_at
                    from 
                        executed_priority_operations, vars
                    where 
                        from_account = address_bytes
                        or
                        to_account = address_bytes
            ) t
            order by
                created_at asc
            limit 
                1
            ",
            address = hex::encode(address.as_ref().to_vec()),
        );
        let tx_history: Vec<AccountCreatedAt> = diesel::sql_query(query).load(self.0.conn())?;

        match tx_history.len() {
            0 => Ok(None),
            1 => {
                let naive_created_at = tx_history[0].created_at;
                let created_at = DateTime::from_utc(naive_created_at, Utc);

                Ok(Some(created_at))
            }
            other => {
                panic!(
                    "Query was limited to 1 entry at max, but returned {} results",
                    other
                );
            }
        }
    }

    /// Loads the range of the transactions applied to the account starting
    /// from the block with number $(offset) up to $(offset + limit).
    pub fn get_account_transactions_history(
        &self,
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
        let query = format!(
            "
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
                    with vars (address_bytes) as ( select decode('{address}', 'hex') )
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
                        executed_transactions, vars
                    where
                        from_account = address_bytes
                        or
                        to_account = address_bytes
                        or
                        primary_account_address = address_bytes
                    union all
                    select
                        concat_ws(',', block_number, block_index) as tx_id,
                        operation as tx,
                        '0x' || encode(eth_hash, 'hex') as hash,
                        priority_op_serialid as pq_id,
                        eth_block,
                        null as success,
                        null as fail_reason,
                        block_number,
                        created_at
                    from 
                        executed_priority_operations, vars
                    where 
                        from_account = address_bytes
                        or
                        to_account = address_bytes) t
                order by
                    block_number desc, created_at desc
                offset 
                    {offset}
                limit 
                    {limit}
            )
            select
                tx_id,
                hash,
                eth_block,
                pq_id,
                tx,
                success,
                fail_reason,
                true as commited,
                coalesce(verified.confirmed, false) as verified,
                created_at
            from transactions
            left join eth_ops verified on
                verified.block_number = transactions.block_number and verified.action_type = 'VERIFY' and verified.confirmed = true
            order by transactions.block_number desc, created_at desc
            ",
            address = hex::encode(address.as_ref().to_vec()),
            offset = offset,
            limit = limit
        );
        let mut tx_history =
            diesel::sql_query(query).load::<TransactionsHistoryItem>(self.0.conn())?;
        if !tx_history.is_empty() {
            let tokens = TokensSchema(self.0).load_tokens()?;
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
    pub fn get_account_transactions_history_from(
        &self,
        address: &Address,
        tx_id: (u64, u64),
        direction: SearchDirection,
        limit: u64,
    ) -> QueryResult<Vec<TransactionsHistoryItem>> {
        let direction_sign = match direction {
            SearchDirection::Older => "<", // Older blocks have lesser block ID.
            SearchDirection::Newer => ">", // Newer blocks have greater block ID.
        };

        // Filter for txs that older/newer than provided tx ID.
        let ordered_filter = format!(
            "(block_number {sign} {block_id} or (block_number = {block_id} and block_index {sign} {block_tx_id}))",
            sign = direction_sign,
            block_id = tx_id.0,
            block_tx_id = tx_id.1
        );

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
        let query = format!(
            "
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
                    with vars (address_bytes) as ( select decode('{address}', 'hex') )
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
                        executed_transactions, vars
                    where
                        (
                            from_account = address_bytes
                            or
                            to_account = address_bytes
                            or
                            primary_account_address = address_bytes
                        )
                        and
                        {ordered_filter}
                    union all
                    select
                        concat_ws(',', block_number, block_index) as tx_id,
                        operation as tx,
                        '0x' || encode(eth_hash, 'hex') as hash,
                        priority_op_serialid as pq_id,
                        eth_block,
                        null as success,
                        null as fail_reason,
                        block_number,
                        created_at
                    from 
                        executed_priority_operations, vars
                    where 
                        (
                            from_account = address_bytes
                            or
                            to_account = address_bytes
                        )
                        and
                        {ordered_filter}
                    ) t
                order by
                    block_number desc, created_at desc
                limit 
                    {limit}
            )
            select
                tx_id,
                hash,
                eth_block,
                pq_id,
                tx,
                success,
                fail_reason,
                true as commited,
                coalesce(verified.confirmed, false) as verified,
                created_at
            from transactions
            left join eth_ops committed on
                committed.block_number = transactions.block_number and committed.action_type = 'COMMIT'
            left join eth_ops verified on
                verified.block_number = transactions.block_number and verified.action_type = 'VERIFY' and verified.confirmed = true
            order by transactions.block_number desc, created_at desc
            ",
            address = hex::encode(address.as_ref().to_vec()),
            ordered_filter = ordered_filter,
            limit = limit
        );
        let mut tx_history =
            diesel::sql_query(query).load::<TransactionsHistoryItem>(self.0.conn())?;
        if !tx_history.is_empty() {
            let tokens = TokensSchema(self.0).load_tokens()?;
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
