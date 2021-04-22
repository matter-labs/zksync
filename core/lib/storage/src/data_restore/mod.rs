// Built-in deps
use std::time::Instant;
// External imports
use itertools::Itertools;
use zksync_basic_types::H256;
// Workspace imports
use zksync_types::{AccountId, AccountUpdate, BlockNumber, Token, ZkSyncOp};
// Local imports
use self::records::{
    NewBlockEvent, NewStorageState, NewTokenEvent, NewZkSyncOp, StoredBlockEvent,
    StoredLastWatchedEthBlockNumber, StoredRollupOpsBlock, StoredStorageState, StoredZkSyncOp,
};

use crate::chain::operations::OperationsSchema;
use crate::{chain::state::StateSchema, tokens::TokensSchema};
use crate::{QueryResult, StorageProcessor};
use zksync_types::aggregated_operations::{
    AggregatedActionType, AggregatedOperation, BlocksCommitOperation, BlocksExecuteOperation,
};

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
        genesis_acc_update: AccountUpdate,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;
        StateSchema(&mut transaction)
            .commit_state_update(BlockNumber(0), &[(AccountId(0), genesis_acc_update)], 0)
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
        let stored_operations = sqlx::query_as!(
            StoredZkSyncOp,
            "SELECT * FROM data_restore_rollup_ops
            ORDER BY id ASC"
        )
        .fetch_all(self.0.conn())
        .await?;

        let ops_blocks: Vec<StoredRollupOpsBlock> = stored_operations
            .into_iter()
            .group_by(|op| op.block_num)
            .into_iter()
            .map(|(_, stored_ops)| {
                // let stored_ops = group.clone();
                // let mut ops: Vec<ZkSyncOp> = vec![];
                let mut block_num: i64 = 0;
                let mut fee_account: i64 = 0;
                let mut timestamp: Option<u64> = None;
                let mut previous_block_root_hash: H256 = H256::default();
                let ops: Vec<ZkSyncOp> = stored_ops
                    .map(|stored_op| {
                        block_num = stored_op.block_num;
                        fee_account = stored_op.fee_account;
                        timestamp = stored_op.timestamp.map(|t| t as u64);
                        previous_block_root_hash = stored_op
                            .previous_block_root_hash
                            .clone()
                            .map(|h| H256::from_slice(&h))
                            .unwrap_or_default();
                        stored_op.into_franklin_op()
                    })
                    .collect();
                StoredRollupOpsBlock {
                    block_num: BlockNumber(block_num as u32),
                    ops,
                    fee_account: AccountId(fee_account as u32),
                    timestamp,
                    previous_block_root_hash,
                }
            })
            .collect();
        metrics::histogram!("sql.data_restore.load_rollup_ops_blocks", start.elapsed());
        Ok(ops_blocks)
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
            TokensSchema(&mut transaction).store_token(token).await?;
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
        ops: &[(BlockNumber, &ZkSyncOp, AccountId, Option<u64>, H256)],
    ) -> QueryResult<()> {
        let start = Instant::now();
        let new_state = self.new_storage_state("Operations");
        let mut transaction = self.0.start_transaction().await?;
        sqlx::query!("DELETE FROM data_restore_rollup_ops")
            .execute(transaction.conn())
            .await?;

        for op in ops.iter() {
            let stored_op =
                NewZkSyncOp::prepare_stored_op(&op.1, op.0, op.2, op.3, op.4.as_bytes().to_vec());

            sqlx::query!(
                "INSERT INTO data_restore_rollup_ops (block_num, operation, fee_account, timestamp, previous_block_root_hash) VALUES ($1, $2, $3, $4, $5)",
                stored_op.block_num, stored_op.operation, stored_op.fee_account, stored_op.timestamp, stored_op.previous_block_root_hash
            ).execute(transaction.conn())
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
