// Built-in deps
// External imports
use diesel::prelude::*;
use itertools::Itertools;
use serde_json::value::Value;
// Workspace imports
use models::node::PubKeyHash;
use models::ActionType;
// Local imports
use self::records::{
    AccountTransaction, PriorityOpReceiptResponse, ReadTx, StoredExecutedTransaction,
    TransactionsHistoryItem, TxByHashResponse, TxReceiptResponse,
};
use crate::interfaces::{
    operations::records::{StoredExecutedPriorityOperation, StoredOperation},
    prover::records::ProverRun,
};
use crate::schema::*;
use crate::StorageProcessor;

pub mod records;

pub trait TransactionsInterface {
    fn tx_receipt(&self, hash: &[u8]) -> QueryResult<Option<TxReceiptResponse>>;

    fn get_priority_op_receipt(&self, op_id: i64) -> QueryResult<PriorityOpReceiptResponse>;

    fn get_tx_by_hash(&self, hash: &[u8]) -> QueryResult<Option<TxByHashResponse>>;

    fn get_account_transactions_history(
        &self,
        address: &PubKeyHash,
        offset: i64,
        limit: i64,
    ) -> QueryResult<Vec<TransactionsHistoryItem>>;

    fn get_account_transactions(
        &self,
        address: &PubKeyHash,
    ) -> QueryResult<Vec<AccountTransaction>>;

    fn get_executed_priority_op(
        &self,
        priority_op_id: u32,
    ) -> QueryResult<Option<StoredExecutedPriorityOperation>>;
}

impl TransactionsInterface for StorageProcessor {
    fn tx_receipt(&self, hash: &[u8]) -> QueryResult<Option<TxReceiptResponse>> {
        self.conn().transaction(|| {
            let tx = executed_transactions::table
                .filter(executed_transactions::tx_hash.eq(hash))
                .first::<StoredExecutedTransaction>(self.conn())
                .optional()?;

            if let Some(tx) = tx {
                let commited = operations::table
                    .filter(operations::block_number.eq(tx.block_number))
                    .filter(operations::action_type.eq(ActionType::COMMIT.to_string()))
                    .first::<StoredOperation>(self.conn())
                    .optional()?
                    .is_some();

                if !commited {
                    return Ok(None);
                }

                let verified = operations::table
                    .filter(operations::block_number.eq(tx.block_number))
                    .filter(operations::action_type.eq(ActionType::VERIFY.to_string()))
                    .first::<StoredOperation>(self.conn())
                    .optional()?
                    .map(|v| v.confirmed)
                    .unwrap_or(false);

                let prover_run: Option<ProverRun> = prover_runs::table
                    .filter(prover_runs::block_number.eq(tx.block_number))
                    .first::<ProverRun>(self.conn())
                    .optional()?;

                Ok(Some(TxReceiptResponse {
                    tx_hash: hex::encode(&hash),
                    block_number: tx.block_number,
                    success: tx.success,
                    verified,
                    fail_reason: tx.fail_reason,
                    prover_run,
                }))
            } else {
                Ok(None)
            }
        })
    }

    fn get_priority_op_receipt(&self, op_id: i64) -> QueryResult<PriorityOpReceiptResponse> {
        // TODO: jazzandrock maybe use one db query(?).
        let stored_executed_prior_op = executed_priority_operations::table
            .filter(executed_priority_operations::priority_op_serialid.eq(op_id))
            .first::<StoredExecutedPriorityOperation>(self.conn())
            .optional()?;

        match stored_executed_prior_op {
            Some(stored_executed_prior_op) => {
                let prover_run: Option<ProverRun> = prover_runs::table
                    .filter(prover_runs::block_number.eq(stored_executed_prior_op.block_number))
                    .first::<ProverRun>(self.conn())
                    .optional()?;

                let commit = operations::table
                    .filter(operations::block_number.eq(stored_executed_prior_op.block_number))
                    .filter(operations::action_type.eq(ActionType::COMMIT.to_string()))
                    .first::<StoredOperation>(self.conn())
                    .optional()?;

                let confirm = operations::table
                    .filter(operations::block_number.eq(stored_executed_prior_op.block_number))
                    .filter(operations::action_type.eq(ActionType::VERIFY.to_string()))
                    .first::<StoredOperation>(self.conn())
                    .optional()?;

                Ok(PriorityOpReceiptResponse {
                    committed: commit.is_some(),
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

    fn get_tx_by_hash(&self, hash: &[u8]) -> QueryResult<Option<TxByHashResponse>> {
        // TODO: Maybe move the transformations to api_server?

        // first check executed_transactions
        let tx: Option<StoredExecutedTransaction> = executed_transactions::table
            .filter(executed_transactions::tx_hash.eq(hash))
            .first(self.conn())
            .optional()?;

        if let Some(tx) = tx {
            let block_number = tx.block_number;
            let operation = tx.operation.unwrap_or_else(|| {
                log::debug!("operation empty in executed_transactions");
                Value::default()
            });

            let tx_type = operation["type"].as_str().unwrap_or("unknown type");
            let tx_token = operation["tx"]["token"].as_i64().unwrap_or(-1);
            let tx_amount = operation["tx"]["amount"]
                .as_str()
                .unwrap_or("unknown amount");

            let (tx_from, tx_to, tx_fee) = match tx_type {
                "Withdraw" => (
                    operation["tx"]["from"]
                        .as_str()
                        .unwrap_or("unknown from")
                        .to_string(),
                    operation["tx"]["to"]
                        .as_str()
                        .unwrap_or("unknown to")
                        .to_string(),
                    operation["tx"]["fee"].as_str().map(|v| v.to_string()),
                ),
                "Transfer" | "TransferToNew" => (
                    operation["tx"]["from"]
                        .as_str()
                        .unwrap_or("unknown from")
                        .to_string(),
                    operation["tx"]["to"]
                        .as_str()
                        .unwrap_or("unknown to")
                        .to_string(),
                    operation["tx"]["fee"].as_str().map(|v| v.to_string()),
                ),
                "ChangePubKeyOffchain" => (
                    operation["tx"]["account"]
                        .as_str()
                        .unwrap_or("unknown from")
                        .to_string(),
                    operation["tx"]["newPkHash"]
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
            }));
        };

        // then check executed_priority_operations
        let tx: Option<StoredExecutedPriorityOperation> = executed_priority_operations::table
            .filter(executed_priority_operations::eth_hash.eq(hash))
            .first(self.conn())
            .optional()?;

        if let Some(tx) = tx {
            let operation = tx.operation;
            let block_number = tx.block_number;

            let tx_type = operation["type"].as_str().unwrap_or("unknown type");
            let tx_token = operation["priority_op"]["token"]
                .as_i64()
                .expect("must be here");
            let tx_amount = operation["priority_op"]["amount"]
                .as_str()
                .unwrap_or("unknown amount");

            let (tx_from, tx_to, tx_fee) = match tx_type {
                "Deposit" => (
                    operation["priority_op"]["from"]
                        .as_str()
                        .unwrap_or("unknown from")
                        .to_string(),
                    operation["priority_op"]["to"]
                        .as_str()
                        .unwrap_or("unknown to")
                        .to_string(),
                    operation["priority_op"]["fee"]
                        .as_str()
                        .map(|v| v.to_string()),
                ),
                &_ => (
                    "unknown from".to_string(),
                    "unknown to".to_string(),
                    Some("unknown fee".to_string()),
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
            }));
        };

        Ok(None)
    }

    fn get_account_transactions_history(
        &self,
        address: &PubKeyHash,
        offset: i64,
        limit: i64,
    ) -> QueryResult<Vec<TransactionsHistoryItem>> {
        // TODO: txs are not ordered
        let query = format!(
            "
            select
                hash,
                pq_id,
                tx,
                success,
                fail_reason,
                coalesce(commited, false) as commited,
                coalesce(verified, false) as verified
            from (
                select
                    *
                from (
                    select
                        tx,
                        'sync-tx:' || encode(hash, 'hex') as hash,
                        null as pq_id,
                        success,
                        fail_reason,
                        block_number
                    from
                        mempool
                    left join
                        executed_transactions
                    on
                        tx_hash = hash
                    where
                        'sync:' || encode(primary_account_address, 'hex') = '{address}'
                        or
                        tx->>'to' = '{address}'
                    union all
                    select
                        operation as tx,
                        '0x' || encode(eth_hash, 'hex') as hash,
                        priority_op_serialid as pq_id,
                        null as success,
                        null as fail_reason,
                        block_number
                    from 
                        executed_priority_operations
                    where 
                        operation->'priority_op'->>'account' = '{address}') t
                order by
                    block_number desc
                offset 
                    {offset}
                limit 
                    {limit}
            ) t
            left join
                crosstab($$
                    select 
                        block_number as rowid, 
                        action_type as category, 
                        true as values 
                    from 
                        operations
                    order by
                        block_number
                    $$) t3 (
                        block_number bigint, 
                        commited boolean, 
                        verified boolean)
            using 
                (block_number)
            ",
            address = address.to_hex(),
            offset = offset,
            limit = limit
        );

        diesel::sql_query(query).load::<TransactionsHistoryItem>(self.conn())
    }

    fn get_account_transactions(
        &self,
        address: &PubKeyHash,
    ) -> QueryResult<Vec<AccountTransaction>> {
        let all_txs: Vec<_> = mempool::table
            .filter(mempool::primary_account_address.eq(address.data.to_vec()))
            .left_join(
                executed_transactions::table.on(executed_transactions::tx_hash.eq(mempool::hash)),
            )
            .left_join(
                operations::table
                    .on(operations::block_number.eq(executed_transactions::block_number)),
            )
            .load::<(
                ReadTx,
                Option<StoredExecutedTransaction>,
                Option<StoredOperation>,
            )>(self.conn())?;

        let res = all_txs
            .into_iter()
            .group_by(|(mempool_tx, _, _)| mempool_tx.hash.clone())
            .into_iter()
            .map(|(_op_id, mut group_iter)| {
                // TODO: replace the query with pivot
                let (mempool_tx, executed_tx, operation) = group_iter.next().unwrap();
                let mut res = AccountTransaction {
                    tx: mempool_tx.tx,
                    tx_hash: hex::encode(mempool_tx.hash.as_slice()),
                    success: false,
                    fail_reason: None,
                    committed: false,
                    verified: false,
                };
                if let Some(executed_tx) = executed_tx {
                    res.success = executed_tx.success;
                    res.fail_reason = executed_tx.fail_reason;
                }
                if let Some(operation) = operation {
                    if operation.action_type == ActionType::COMMIT.to_string() {
                        res.committed = operation.confirmed;
                    } else {
                        res.verified = operation.confirmed;
                    }
                }
                if let Some((_mempool_tx, _executed_tx, operation)) = group_iter.next() {
                    if let Some(operation) = operation {
                        if operation.action_type == ActionType::COMMIT.to_string() {
                            res.committed = operation.confirmed;
                        } else {
                            res.verified = operation.confirmed;
                        }
                    };
                }
                res
            })
            .collect::<Vec<AccountTransaction>>();

        Ok(res)
    }

    fn get_executed_priority_op(
        &self,
        priority_op_id: u32,
    ) -> QueryResult<Option<StoredExecutedPriorityOperation>> {
        executed_priority_operations::table
            .filter(
                executed_priority_operations::priority_op_serialid.eq(i64::from(priority_op_id)),
            )
            .first::<StoredExecutedPriorityOperation>(self.conn())
            .optional()
    }
}
