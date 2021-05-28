// Built-in deps
use std::time::Instant;
// External imports
// Workspace imports
use zksync_types::{
    aggregated_operations::{
        AggregatedActionType, AggregatedOperation, BlocksCommitOperation, BlocksExecuteOperation,
    },
    AccountId, AccountUpdate, BlockNumber, Token,
};
// Local imports
use self::records::{
    NewBlockEvent, NewRollupOpsBlock, NewStorageState, NewTokenEvent, StoredBlockEvent,
    StoredLastWatchedEthBlockNumber, StoredRollupOpsBlock, StoredStorageState,
};

use crate::chain::operations::OperationsSchema;
use crate::{
    chain::state::StateSchema,
    tokens::{StoreTokenError, TokensSchema},
};
use crate::{QueryResult, StorageProcessor};

pub mod records;

/// Data restore schema provides a convenient interface to restore the
/// sidechain state from the Ethereum contract.
///
/// This schema is used exclusively by the `data_restore` crate.
#[derive(Debug)]
pub struct DataRestoreSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> DataRestoreSchema<'a, 'c> {
    pub async fn save_block_operations(
        &mut self,
        commit_op: BlocksCommitOperation,
        execute_op: BlocksExecuteOperation,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let new_state = self.new_storage_state("None");
        let mut transaction = self.0.start_transaction().await?;

        OperationsSchema(&mut transaction)
            .store_aggregated_action(AggregatedOperation::CommitBlocks(commit_op.clone()))
            .await?;
        OperationsSchema(&mut transaction)
            .store_aggregated_action(AggregatedOperation::ExecuteBlocks(execute_op.clone()))
            .await?;
        // The state is expected to be updated, so it's necessary
        // to do it here.
        for block in commit_op.blocks.iter() {
            StateSchema(&mut transaction)
                .apply_state_update(block.block_number)
                .await?;
        }

        OperationsSchema(&mut transaction)
            .confirm_aggregated_operations(
                commit_op.blocks.first().unwrap().block_number,
                commit_op.blocks.last().unwrap().block_number,
                AggregatedActionType::CommitBlocks,
            )
            .await?;

        OperationsSchema(&mut transaction)
            .confirm_aggregated_operations(
                execute_op.blocks.first().unwrap().block_number,
                execute_op.blocks.last().unwrap().block_number,
                AggregatedActionType::ExecuteBlocks,
            )
            .await?;

        DataRestoreSchema(&mut transaction)
            .update_storage_state(new_state)
            .await?;
        transaction.commit().await?;
        metrics::histogram!("sql.data_restore.save_block_operations", start.elapsed());
        Ok(())
    }

    pub async fn save_genesis_state(
        &mut self,
        genesis_updates: &[(AccountId, AccountUpdate)],
    ) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;
        StateSchema(&mut transaction)
            .commit_state_update(BlockNumber(0), genesis_updates, 0)
            .await?;
        StateSchema(&mut transaction)
            .apply_state_update(BlockNumber(0))
            .await?;
        transaction.commit().await?;
        metrics::histogram!("sql.data_restore.save_genesis_state", start.elapsed());
        Ok(())
    }

    pub async fn load_rollup_ops_blocks(&mut self) -> QueryResult<Vec<StoredRollupOpsBlock>> {
        let start = Instant::now();
        // For each block aggregate its operations from the
        // `data_restore_rollup_block_ops` table into array and
        // match it by the block number from `data_restore_rollup_blocks`.
        // The contract version is obtained from block events.
        let stored_blocks = sqlx::query_as!(
            StoredRollupOpsBlock,
            "SELECT blocks.block_num AS block_num, ops, fee_account,
            timestamp, previous_block_root_hash, contract_version
            FROM data_restore_rollup_blocks AS blocks
            JOIN (
                SELECT block_num, array_agg(operation) as ops
                FROM data_restore_rollup_block_ops
                GROUP BY block_num
            ) ops
                ON blocks.block_num = ops.block_num
            JOIN data_restore_events_state as events
                ON blocks.block_num = events.block_num
            ORDER BY blocks.block_num ASC"
        )
        .fetch_all(self.0.conn())
        .await?;
        metrics::histogram!("sql.data_restore.load_rollup_ops_blocks", start.elapsed());
        Ok(stored_blocks)
    }

    /// Stores the last seen Ethereum block number.
    pub(crate) async fn update_last_watched_block_number(
        &mut self,
        block_number: &str,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;
        sqlx::query!("DELETE FROM data_restore_last_watched_eth_block")
            .execute(transaction.conn())
            .await?;

        sqlx::query!(
            "INSERT INTO data_restore_last_watched_eth_block (block_number) VALUES ($1)",
            block_number
        )
        .execute(transaction.conn())
        .await?;
        transaction.commit().await?;

        metrics::histogram!(
            "sql.data_restore.update_last_watched_block_number",
            start.elapsed()
        );
        Ok(())
    }

    /// Loads the last seen Ethereum block number.
    pub async fn load_last_watched_block_number(
        &mut self,
    ) -> QueryResult<StoredLastWatchedEthBlockNumber> {
        let start = Instant::now();
        let stored = sqlx::query_as!(
            StoredLastWatchedEthBlockNumber,
            "SELECT * FROM data_restore_last_watched_eth_block LIMIT 1",
        )
        .fetch_one(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.data_restore.load_last_watched_block_number",
            start.elapsed()
        );
        Ok(stored)
    }

    fn new_storage_state(&self, state: impl ToString) -> NewStorageState {
        NewStorageState {
            storage_state: state.to_string(),
        }
    }

    pub async fn save_events_state(
        &mut self,
        block_events: &[NewBlockEvent],
        token_events: &[NewTokenEvent],
        last_watched_eth_number: &str,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let new_state = self.new_storage_state("Events");
        let mut transaction = self.0.start_transaction().await?;
        DataRestoreSchema(&mut transaction)
            .update_block_events(block_events)
            .await?;

        for &NewTokenEvent { id, address } in token_events.iter() {
            // The only way to know decimals is to query ERC20 contract 'decimals' function
            // that may or may not (in most cases, may not) be there, so we just assume it to be 18
            let decimals = 18;
            let token = Token::new(id, address, &format!("ERC20-{}", *id), decimals);
            let try_insert_token = TokensSchema(&mut transaction).store_token(token).await;

            if let Err(StoreTokenError::Other(anyhow_err)) = try_insert_token {
                return Err(anyhow_err);
            }
        }

        DataRestoreSchema(&mut transaction)
            .update_last_watched_block_number(last_watched_eth_number)
            .await?;
        DataRestoreSchema(&mut transaction)
            .update_storage_state(new_state)
            .await?;

        transaction.commit().await?;

        metrics::histogram!("sql.data_restore.save_events_state", start.elapsed());
        Ok(())
    }

    pub async fn save_rollup_ops(
        &mut self,
        rollup_blocks: &[NewRollupOpsBlock<'_>],
    ) -> QueryResult<()> {
        let start = Instant::now();
        let new_state = self.new_storage_state("Operations");
        let mut transaction = self.0.start_transaction().await?;
        // Clean up the blocks table. Operations will be removed too since there
        // is a foreign-key constraint on the block number.
        sqlx::query!("DELETE FROM data_restore_rollup_blocks")
            .execute(transaction.conn())
            .await?;

        for block in rollup_blocks {
            sqlx::query!(
                "INSERT INTO data_restore_rollup_blocks
                VALUES ($1, $2, $3, $4)",
                i64::from(*block.block_num),
                i64::from(*block.fee_account),
                block.timestamp.map(|t| t as i64),
                Some(block.previous_block_root_hash.as_bytes().to_vec())
            )
            .execute(transaction.conn())
            .await?;

            let operations: Vec<_> = block
                .ops
                .iter()
                .map(|op| serde_json::to_value(op.clone()).unwrap())
                .collect();
            sqlx::query!(
                "INSERT INTO data_restore_rollup_block_ops (block_num, operation)
                SELECT $1, u.operation
                    FROM UNNEST ($2::jsonb[])
                    AS u(operation)",
                i64::from(*block.block_num),
                &operations,
            )
            .execute(transaction.conn())
            .await?;
        }
        DataRestoreSchema(&mut transaction)
            .update_storage_state(new_state)
            .await?;
        transaction.commit().await?;
        metrics::histogram!("sql.data_restore.save_rollup_ops", start.elapsed());
        Ok(())
    }

    /// Method that initializes the `eth_stats` table.
    /// Since `eth_sender` module uses this table to identify the expected next block numbers
    /// for sending operations to the Ethereum, we must initialize it with actual values.
    pub async fn initialize_eth_stats(
        &mut self,
        last_committed_block: BlockNumber,
        last_verified_block: BlockNumber,
        last_executed_block: BlockNumber,
    ) -> QueryResult<()> {
        let start = Instant::now();

        sqlx::query!(
            "UPDATE eth_parameters
            SET last_committed_block = $1, last_verified_block = $2, last_executed_block = $3
            WHERE id = true",
            *last_committed_block as i64,
            *last_verified_block as i64,
            *last_executed_block as i64
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.data_restore.initialize_eth_stats", start.elapsed());
        Ok(())
    }

    async fn load_events_state(&mut self, state: &str) -> QueryResult<Vec<StoredBlockEvent>> {
        let start = Instant::now();
        let events = sqlx::query_as!(
            StoredBlockEvent,
            "SELECT * FROM data_restore_events_state
            WHERE block_type = $1
            ORDER BY block_num ASC",
            state,
        )
        .fetch_all(self.0.conn())
        .await?;

        metrics::histogram!("sql.data_restore.load_events_state", start.elapsed());
        Ok(events)
    }

    pub async fn load_committed_events_state(&mut self) -> QueryResult<Vec<StoredBlockEvent>> {
        self.load_events_state("Committed").await
    }

    pub async fn load_verified_events_state(&mut self) -> QueryResult<Vec<StoredBlockEvent>> {
        self.load_events_state("Verified").await
    }

    pub async fn load_storage_state(&mut self) -> QueryResult<StoredStorageState> {
        let start = Instant::now();
        let state = sqlx::query_as!(
            StoredStorageState,
            "SELECT * FROM data_restore_storage_state_update
            LIMIT 1",
        )
        .fetch_one(self.0.conn())
        .await?;

        metrics::histogram!("sql.data_restore.load_storage_state", start.elapsed());
        Ok(state)
    }

    pub(crate) async fn update_storage_state(&mut self, state: NewStorageState) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;
        sqlx::query!("DELETE FROM data_restore_storage_state_update")
            .execute(transaction.conn())
            .await?;

        sqlx::query!(
            "INSERT INTO data_restore_storage_state_update (storage_state) VALUES ($1)",
            state.storage_state,
        )
        .execute(transaction.conn())
        .await?;
        transaction.commit().await?;

        metrics::histogram!("sql.data_restore.update_storage_state", start.elapsed());
        Ok(())
    }

    pub(crate) async fn update_block_events(
        &mut self,
        events: &[NewBlockEvent],
    ) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;
        sqlx::query!("DELETE FROM data_restore_events_state")
            .execute(transaction.conn())
            .await?;

        for event in events.iter() {
            sqlx::query!(
                "INSERT INTO data_restore_events_state (block_type, transaction_hash, block_num, contract_version) VALUES ($1, $2, $3, $4)",
                event.block_type, event.transaction_hash, event.block_num, event.contract_version
            )
            .execute(transaction.conn())
            .await?;
        }
        transaction.commit().await?;
        metrics::histogram!("sql.data_restore.update_block_events", start.elapsed());
        Ok(())
    }
}
