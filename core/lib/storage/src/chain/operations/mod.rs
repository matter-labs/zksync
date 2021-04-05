// Built-in deps
use std::time::Instant;
// External imports
use chrono::{Duration, Utc};
// Workspace imports
use zksync_types::{tx::TxHash, BlockNumber};
// Local imports
use self::records::{
    NewExecutedPriorityOperation, NewExecutedTransaction, StoredAggregatedOperation,
    StoredCompleteWithdrawalsTransaction, StoredExecutedPriorityOperation, StoredPendingWithdrawal,
};
use crate::chain::operations::records::StoredExecutedTransaction;
use crate::chain::operations_ext::OperationsExtSchema;
use crate::ethereum::EthereumSchema;
use crate::{chain::mempool::MempoolSchema, QueryResult, StorageProcessor};
use zksync_basic_types::H256;
use zksync_types::aggregated_operations::{AggregatedActionType, AggregatedOperation};

pub mod records;

/// Operations schema is capable of storing and loading the transactions.
/// Every kind of transaction (non-executed, executed, and executed priority tx)
/// can be either saved or loaded from the database.
#[derive(Debug)]
pub struct OperationsSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> OperationsSchema<'a, 'c> {
    /// Return the greatest block number with the given `action_type` and `confirmed` status.
    pub async fn get_last_block_by_aggregated_action(
        &mut self,
        aggregated_action_type: AggregatedActionType,
        confirmed: Option<bool>,
    ) -> QueryResult<BlockNumber> {
        let start = Instant::now();
        let max_block = sqlx::query!(
            r#"SELECT max(to_block) FROM aggregate_operations WHERE action_type = $1 AND confirmed IS DISTINCT FROM $2"#,
            aggregated_action_type.to_string(),
            confirmed.map(|value| !value)
        )
        .fetch_one(self.0.conn())
        .await?
        .max
        .unwrap_or(0);

        metrics::histogram!(
            "sql.chain.operations.get_last_block_by_aggregated_action",
            start.elapsed()
        );
        Ok(BlockNumber(max_block as u32))
    }

    pub async fn get_stored_aggregated_operation(
        &mut self,
        block_number: BlockNumber,
        aggregated_action_type: AggregatedActionType,
    ) -> Option<StoredAggregatedOperation> {
        let start = Instant::now();
        let result = sqlx::query_as!(
            StoredAggregatedOperation,
            "SELECT * FROM aggregate_operations WHERE from_block >= $1 AND to_block <= $1 AND action_type = $2",
            i64::from(*block_number),
            aggregated_action_type.to_string()
        )
        .fetch_optional(self.0.conn())
        .await
        .ok()
        .flatten();

        metrics::histogram!(
            "sql.chain.operations.get_stored_aggregated_operations",
            start.elapsed()
        );
        result
    }

    /// Retrieves transaction from the database given its hash.
    pub async fn get_executed_operation(
        &mut self,
        op_hash: &[u8],
    ) -> QueryResult<Option<StoredExecutedTransaction>> {
        let start = Instant::now();
        let op = sqlx::query_as!(
            StoredExecutedTransaction,
            "SELECT * FROM executed_transactions WHERE tx_hash = $1",
            op_hash
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.operations.get_executed_operation",
            start.elapsed()
        );
        Ok(op)
    }

    /// Retrieves priority operation from the database given its ID.
    pub async fn get_executed_priority_operation(
        &mut self,
        priority_op_id: u32,
    ) -> QueryResult<Option<StoredExecutedPriorityOperation>> {
        let start = Instant::now();
        let op = sqlx::query_as!(
            StoredExecutedPriorityOperation,
            "SELECT * FROM executed_priority_operations WHERE priority_op_serialid = $1",
            i64::from(priority_op_id)
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.operations.get_executed_priority_operation",
            start.elapsed()
        );
        Ok(op)
    }

    /// Retrieves priority operation from the database given its hash.
    pub async fn get_executed_priority_operation_by_hash(
        &mut self,
        eth_hash: &[u8],
    ) -> QueryResult<Option<StoredExecutedPriorityOperation>> {
        let start = Instant::now();
        let op = sqlx::query_as!(
            StoredExecutedPriorityOperation,
            "SELECT * FROM executed_priority_operations WHERE eth_hash = $1",
            eth_hash
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.operations.get_executed_priority_operation_by_hash",
            start.elapsed()
        );
        Ok(op)
    }

    pub async fn confirm_aggregated_operations(
        &mut self,
        first_block: BlockNumber,
        last_block: BlockNumber,
        action_type: AggregatedActionType,
    ) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            "UPDATE aggregate_operations
                SET confirmed = $1
                WHERE from_block >= $2 AND to_block <= $3 AND action_type = $4",
            true,
            i64::from(*first_block),
            i64::from(*last_block),
            action_type.to_string()
        )
        .execute(self.0.conn())
        .await?;
        metrics::histogram!(
            "sql.chain.operations.confirm_aggregated_operations",
            start.elapsed()
        );
        Ok(())
    }

    /// Stores the executed transaction in the database.
    pub(crate) async fn store_executed_tx(
        &mut self,
        operation: NewExecutedTransaction,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        MempoolSchema(&mut transaction)
            .remove_tx(&operation.tx_hash)
            .await?;

        if operation.success {
            // If transaction succeed, it should replace the stored tx with the same hash.
            // The situation when a duplicate tx is stored in the database may exist only if has
            // failed previously.
            // Possible scenario: user had no enough funds for transfer, then deposited some and
            // sent the same transfer again.

            sqlx::query!(
                "INSERT INTO executed_transactions (block_number, block_index, tx, operation, tx_hash, from_account, to_account, success, fail_reason, primary_account_address, nonce, created_at, eth_sign_data, batch_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                ON CONFLICT (tx_hash)
                DO UPDATE
                SET block_number = $1, block_index = $2, tx = $3, operation = $4, tx_hash = $5, from_account = $6, to_account = $7, success = $8, fail_reason = $9, primary_account_address = $10, nonce = $11, created_at = $12, eth_sign_data = $13, batch_id = $14",
                operation.block_number,
                operation.block_index,
                operation.tx,
                operation.operation,
                operation.tx_hash,
                operation.from_account,
                operation.to_account,
                operation.success,
                operation.fail_reason,
                operation.primary_account_address,
                operation.nonce,
                operation.created_at,
                operation.eth_sign_data,
                operation.batch_id,
            )
            .execute(transaction.conn())
            .await?;
        } else {
            // If transaction failed, we do nothing on conflict.
            sqlx::query!(
                "INSERT INTO executed_transactions (block_number, block_index, tx, operation, tx_hash, from_account, to_account, success, fail_reason, primary_account_address, nonce, created_at, eth_sign_data, batch_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                ON CONFLICT (tx_hash)
                DO NOTHING",
                operation.block_number,
                operation.block_index,
                operation.tx,
                operation.operation,
                operation.tx_hash,
                operation.from_account,
                operation.to_account,
                operation.success,
                operation.fail_reason,
                operation.primary_account_address,
                operation.nonce,
                operation.created_at,
                operation.eth_sign_data,
                operation.batch_id,
            )
            .execute(transaction.conn())
            .await?;
        };

        transaction.commit().await?;
        metrics::histogram!("sql.chain.operations.store_executed_tx", start.elapsed());
        Ok(())
    }

    /// Removes all rejected transactions with an age greater than `max_age` from the database.
    pub async fn remove_rejected_transactions(&mut self, max_age: Duration) -> QueryResult<()> {
        let start = Instant::now();

        let offset = Utc::now() - max_age;
        sqlx::query!(
            "DELETE FROM executed_transactions
            WHERE success = false AND created_at < $1",
            offset
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.operations.remove_rejected_transactions",
            start.elapsed()
        );
        Ok(())
    }

    /// Stores executed priority operation in database.
    ///
    /// This method is made public to fill the database for tests, do not use it for
    /// any other purposes.
    #[doc = "hidden"]
    pub async fn store_executed_priority_op(
        &mut self,
        operation: NewExecutedPriorityOperation,
    ) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            "INSERT INTO executed_priority_operations (block_number, block_index, operation, from_account, to_account, priority_op_serialid, deadline_block, eth_hash, eth_block, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (priority_op_serialid)
            DO NOTHING",
            operation.block_number,
            operation.block_index,
            operation.operation,
            operation.from_account,
            operation.to_account,
            operation.priority_op_serialid,
            operation.deadline_block,
            operation.eth_hash,
            operation.eth_block,
            operation.created_at,
        )
        .execute(self.0.conn())
        .await?;
        metrics::histogram!(
            "sql.chain.operations.store_executed_priority_op",
            start.elapsed()
        );
        Ok(())
    }

    /// On old contracts, a separate operation was used to withdraw - `CompleteWithdrawals`.
    ///
    /// NOTE: Currently `CompleteWithdrawals` is deprecated but the information is still stored
    /// in the database and it is useful to be able to issue an ether hash for this operation.
    async fn eth_withdraw_tx_for_complete_withdrawal(
        &mut self,
        withdrawal_hash: &TxHash,
    ) -> QueryResult<Option<H256>> {
        let start = Instant::now();
        let pending_withdrawal = sqlx::query_as!(
            StoredPendingWithdrawal,
            "SELECT * FROM pending_withdrawals WHERE withdrawal_hash = $1
            LIMIT 1",
            withdrawal_hash.as_ref().to_vec(),
        )
        .fetch_optional(self.0.conn())
        .await?;

        let res = match pending_withdrawal {
            Some(pending_withdrawal) => {
                let pending_withdrawal_id = pending_withdrawal.id;

                sqlx::query_as!(
                    StoredCompleteWithdrawalsTransaction,
                    "SELECT * FROM complete_withdrawals_transactions
                        WHERE pending_withdrawals_queue_start_index <= $1
                            AND $1 < pending_withdrawals_queue_end_index
                    LIMIT 1
                    ",
                    pending_withdrawal_id,
                )
                .fetch_optional(self.0.conn())
                .await?
                .map(|complete_withdrawals_transaction| {
                    H256::from_slice(&complete_withdrawals_transaction.tx_hash)
                })
            }
            None => None,
        };

        metrics::histogram!(
            "sql.chain.operations.eth_withdraw_tx_for_complete_withdrawal",
            start.elapsed()
        );
        Ok(res)
    }

    /// On the current version of contracts all withdrawals are made as Internal Transactions in `executeBlocks`, so the hash
    /// of the transaction in which the withdrawals from the contract to the user took place will be the same as `ExecuteBlocks` tx.
    async fn eth_withdraw_tx_for_execute_block(
        &mut self,
        withdrawal_hash: &TxHash,
    ) -> QueryResult<Option<H256>> {
        let start = Instant::now();

        let tx_by_hash = OperationsExtSchema(self.0)
            .get_tx_by_hash(withdrawal_hash.as_ref())
            .await?;
        let block_number = if let Some(tx) = tx_by_hash {
            BlockNumber(tx.block_number as u32)
        } else {
            return Ok(None);
        };

        let withdrawal_hash = EthereumSchema(self.0)
            .aggregated_op_final_hash(block_number)
            .await?;

        metrics::histogram!(
            "sql.chain.operations.eth_withdraw_tx_for_execute_block",
            start.elapsed()
        );
        Ok(withdrawal_hash)
    }

    /// Returns the hash of the Ethereum transaction in which the
    /// funds were withdrawn corresponding to the withdraw operation on L2.
    pub async fn eth_tx_for_withdrawal(
        &mut self,
        withdrawal_hash: &TxHash,
    ) -> QueryResult<Option<H256>> {
        let start = Instant::now();

        // For a long time, the operation `CompleteWithdrawals` was used to withdraw funds,
        // now it is used `ExecuteBlocks`, so we should check each of the possible options.
        let eth_withdraw_tx_for_execute_block = self
            .eth_withdraw_tx_for_execute_block(withdrawal_hash)
            .await?;
        let eth_withdraw_tx_for_complete_withdrawal = self
            .eth_withdraw_tx_for_complete_withdrawal(withdrawal_hash)
            .await?;

        let eth_tx_hash =
            eth_withdraw_tx_for_execute_block.or(eth_withdraw_tx_for_complete_withdrawal);

        metrics::histogram!(
            "sql.chain.operations.eth_tx_for_withdrawal",
            start.elapsed()
        );

        Ok(eth_tx_hash)
    }

    pub async fn store_aggregated_action(
        &mut self,
        operation: AggregatedOperation,
    ) -> QueryResult<()> {
        let mut transaction = self.0.start_transaction().await?;

        let aggregated_action_type = operation.get_action_type();
        let (from_block, to_block) = operation.get_block_range();

        let id = sqlx::query!(
            "INSERT INTO aggregate_operations (action_type, arguments, from_block, to_block)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (id)
            DO NOTHING
            RETURNING id",
            aggregated_action_type.to_string(),
            serde_json::to_value(operation.clone()).expect("aggregated op serialize fail"),
            i64::from(*from_block),
            i64::from(*to_block)
        )
        .fetch_one(transaction.conn())
        .await?
        .id;

        if operation.is_commit() {
            sqlx::query!(
                r#"
                INSERT INTO commit_aggregated_blocks_binding
                SELECT 
                    aggregate_operations.id, blocks.number
                FROM aggregate_operations
                INNER JOIN blocks ON blocks.number BETWEEN aggregate_operations.from_block AND aggregate_operations.to_block
                WHERE aggregate_operations.action_type = 'CommitBlocks' and aggregate_operations.id = $1
                "#, 
                id
            ).execute(transaction.conn()).await?;
        }

        if operation.is_execute() {
            sqlx::query!(
                r#"
                INSERT INTO execute_aggregated_blocks_binding
                SELECT 
                    aggregate_operations.id, blocks.number
                FROM aggregate_operations
                INNER JOIN blocks ON blocks.number BETWEEN aggregate_operations.from_block AND aggregate_operations.to_block
                WHERE aggregate_operations.action_type = 'ExecuteBlocks' and aggregate_operations.id = $1
                "#, 
                id
            ).execute(transaction.conn()).await?;
        }

        if !operation.is_create_proof() {
            sqlx::query!(
                "INSERT INTO eth_unprocessed_aggregated_ops (op_id)
                VALUES ($1)",
                id
            )
            .execute(transaction.conn())
            .await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    pub async fn get_last_affected_block_by_aggregated_action(
        &mut self,
        aggregated_action: AggregatedActionType,
    ) -> QueryResult<BlockNumber> {
        let block_number = sqlx::query!(
            "SELECT max(to_block) from aggregate_operations where action_type = $1",
            aggregated_action.to_string(),
        )
        .fetch_one(self.0.conn())
        .await?
        .max
        .map(|b| BlockNumber(b as u32))
        .unwrap_or_default();
        Ok(block_number)
    }

    pub async fn get_aggregated_op_that_affects_block(
        &mut self,
        aggregated_action: AggregatedActionType,
        block_number: BlockNumber,
    ) -> QueryResult<Option<(i64, AggregatedOperation)>> {
        let aggregated_op = sqlx::query_as!(
            StoredAggregatedOperation,
            "SELECT * FROM aggregate_operations \
            WHERE action_type = $1 and from_block <= $2 and $2 <= to_block",
            aggregated_action.to_string(),
            i64::from(*block_number)
        )
        .fetch_optional(self.0.conn())
        .await?
        .map(|op| {
            (
                op.id,
                serde_json::from_value(op.arguments).expect("unparsable aggregated op"),
            )
        });
        Ok(aggregated_op)
    }

    // Removes ethereum unprocessed aggregated operations
    pub async fn remove_eth_unprocessed_aggregated_ops(&mut self) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!("TRUNCATE eth_unprocessed_aggregated_ops")
            .execute(self.0.conn())
            .await?;

        metrics::histogram!(
            "sql.chain.operations.remove_eth_unprocessed_aggregated_ops",
            start.elapsed()
        );
        Ok(())
    }

    // Removes executed priority operations for blocks with number greater than `last_block`
    pub async fn remove_executed_priority_operations(
        &mut self,
        last_block: BlockNumber,
    ) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            "DELETE FROM executed_priority_operations WHERE block_number > $1",
            *last_block as i64
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.operations.remove_executed_priority_operations",
            start.elapsed()
        );
        Ok(())
    }

    // Removes aggregate operations and bindings for blocks with number greater than `last_block`
    pub async fn remove_aggregate_operations_and_bindings(
        &mut self,
        last_block: BlockNumber,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;
        let op_ids: Vec<i64> = sqlx::query!(
            "SELECT id FROM aggregate_operations WHERE from_block > $1",
            *last_block as i64
        )
        .fetch_all(transaction.conn())
        .await?
        .into_iter()
        .map(|record| record.id)
        .collect();

        let eth_op_ids: Vec<i64> = sqlx::query!(
            "SELECT eth_op_id FROM eth_aggregated_ops_binding WHERE op_id = ANY($1)",
            &op_ids
        )
        .fetch_all(transaction.conn())
        .await?
        .into_iter()
        .map(|record| record.eth_op_id)
        .collect();

        sqlx::query!(
            "DELETE FROM eth_tx_hashes WHERE eth_op_id = ANY($1)",
            &eth_op_ids
        )
        .execute(transaction.conn())
        .await?;
        sqlx::query!(
            "DELETE FROM eth_aggregated_ops_binding WHERE op_id = ANY($1)",
            &op_ids
        )
        .execute(transaction.conn())
        .await?;
        sqlx::query!("DELETE FROM eth_operations WHERE id = ANY($1)", &eth_op_ids)
            .execute(transaction.conn())
            .await?;
        sqlx::query!(
            "DELETE FROM aggregate_operations WHERE from_block > $1",
            *last_block as i64
        )
        .execute(transaction.conn())
        .await?;
        sqlx::query!(
            "UPDATE aggregate_operations SET to_block = $1 WHERE to_block > $1",
            *last_block as i64
        )
        .execute(transaction.conn())
        .await?;
        transaction.commit().await?;

        metrics::histogram!(
            "sql.chain.operations.remove_aggregate_operations_and_bindings",
            start.elapsed()
        );
        Ok(())
    }
}
