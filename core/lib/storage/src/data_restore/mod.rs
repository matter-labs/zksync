// Built-in deps
// External imports
use itertools::Itertools;
// Workspace imports
use zksync_types::block::Block;
use zksync_types::{AccountId, AccountUpdate, ActionType, BlockNumber, Operation, Token, ZkSyncOp};
// Local imports
use self::records::{
    NewBlockEvent, NewStorageState, NewTokenEvent, NewZkSyncOp, StoredBlockEvent,
    StoredLastWatchedEthBlockNumber, StoredRollupOpsBlock, StoredStorageState, StoredZkSyncOp,
};
use crate::{
    chain::{block::BlockSchema, operations::OperationsSchema, state::StateSchema},
    tokens::TokensSchema,
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
    pub async fn save_block_transactions(&mut self, block: Block) -> QueryResult<()> {
        let new_state = self.new_storage_state("None");
        let mut transaction = self.0.start_transaction().await?;

        BlockSchema(&mut transaction)
            .save_block_transactions(block.block_number, block.block_transactions)
            .await?;
        DataRestoreSchema(&mut transaction)
            .update_storage_state(new_state)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn save_block_operations(
        &mut self,
        commit_op: Operation,
        verify_op: Operation,
    ) -> QueryResult<()> {
        let new_state = self.new_storage_state("None");
        let mut transaction = self.0.start_transaction().await?;

        let commit_op = BlockSchema(&mut transaction)
            .execute_operation(commit_op)
            .await?;
        let verify_op = BlockSchema(&mut transaction)
            .execute_operation(verify_op)
            .await?;
        // The state is expected to be updated, so it's necessary
        // to do it here.
        StateSchema(&mut transaction)
            .apply_state_update(verify_op.block.block_number)
            .await?;
        OperationsSchema(&mut transaction)
            .confirm_operation(commit_op.block.block_number, ActionType::COMMIT)
            .await?;
        OperationsSchema(&mut transaction)
            .confirm_operation(verify_op.block.block_number, ActionType::VERIFY)
            .await?;

        DataRestoreSchema(&mut transaction)
            .update_storage_state(new_state)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn save_genesis_state(
        &mut self,
        genesis_acc_update: AccountUpdate,
    ) -> QueryResult<()> {
        let mut transaction = self.0.start_transaction().await?;
        StateSchema(&mut transaction)
            .commit_state_update(0, &[(0, genesis_acc_update)], 0)
            .await?;
        StateSchema(&mut transaction).apply_state_update(0).await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn load_rollup_ops_blocks(&mut self) -> QueryResult<Vec<StoredRollupOpsBlock>> {
        let stored_operations = sqlx::query_as!(
            StoredZkSyncOp,
            "SELECT * FROM data_restore_rollup_ops
            ORDER BY id ASC"
        )
        .fetch_all(self.0.conn())
        .await?;

        // let stored_operations = data_restore_rollup_ops::table
        //     .order(data_restore_rollup_ops::id.asc())
        //     .load::<StoredZkSyncOp>(self.0.conn())?;
        let ops_blocks: Vec<StoredRollupOpsBlock> = stored_operations
            .into_iter()
            .group_by(|op| op.block_num)
            .into_iter()
            .map(|(_, stored_ops)| {
                // let stored_ops = group.clone();
                // let mut ops: Vec<ZkSyncOp> = vec![];
                let mut block_num: i64 = 0;
                let mut fee_account: i64 = 0;
                let ops: Vec<ZkSyncOp> = stored_ops
                    .map(|stored_op| {
                        block_num = stored_op.block_num;
                        fee_account = stored_op.fee_account;
                        stored_op.into_franklin_op()
                    })
                    .collect();
                StoredRollupOpsBlock {
                    block_num: block_num as u32,
                    ops,
                    fee_account: fee_account as u32,
                }
            })
            .collect();
        Ok(ops_blocks)
    }

    /// Stores the last seen Ethereum block number.
    pub(crate) async fn update_last_watched_block_number(
        &mut self,
        block_number: &str,
    ) -> QueryResult<()> {
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

        Ok(())
    }

    /// Loads the last seen Ethereum block number.
    pub async fn load_last_watched_block_number(
        &mut self,
    ) -> QueryResult<StoredLastWatchedEthBlockNumber> {
        let stored = sqlx::query_as!(
            StoredLastWatchedEthBlockNumber,
            "SELECT * FROM data_restore_last_watched_eth_block LIMIT 1",
        )
        .fetch_one(self.0.conn())
        .await?;

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
        let new_state = self.new_storage_state("Events");
        let mut transaction = self.0.start_transaction().await?;
        DataRestoreSchema(&mut transaction)
            .update_block_events(block_events)
            .await?;

        for &NewTokenEvent { id, address } in token_events.iter() {
            // The only way to know decimals is to query ERC20 contract 'decimals' function
            // that may or may not (in most cases, may not) be there, so we just assume it to be 18
            let decimals = 18;
            let token = Token::new(id, address, &format!("ERC20-{}", id), decimals);
            TokensSchema(&mut transaction).store_token(token).await?;
        }

        DataRestoreSchema(&mut transaction)
            .update_last_watched_block_number(last_watched_eth_number)
            .await?;
        DataRestoreSchema(&mut transaction)
            .update_storage_state(new_state)
            .await?;

        transaction.commit().await?;

        Ok(())
    }

    pub async fn save_rollup_ops(
        &mut self,
        ops: &[(BlockNumber, &ZkSyncOp, AccountId)],
    ) -> QueryResult<()> {
        let new_state = self.new_storage_state("Operations");
        let mut transaction = self.0.start_transaction().await?;
        sqlx::query!("DELETE FROM data_restore_rollup_ops")
            .execute(transaction.conn())
            .await?;

        for op in ops.iter() {
            let stored_op = NewZkSyncOp::prepare_stored_op(&op.1, op.0, op.2);

            sqlx::query!(
                "INSERT INTO data_restore_rollup_ops (block_num, operation, fee_account) VALUES ($1, $2, $3)",
                stored_op.block_num, stored_op.operation, stored_op.fee_account
            ).execute(transaction.conn())
                .await?;
        }
        DataRestoreSchema(&mut transaction)
            .update_storage_state(new_state)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Method that initializes the `eth_stats` table.
    /// Since `eth_sender` module uses this table to identify the expected next block numbers
    /// for sending operations to the Ethereum, we must initialize it with actual values.
    pub async fn initialize_eth_stats(
        &mut self,
        last_committed_block: BlockNumber,
        last_verified_block: BlockNumber,
    ) -> QueryResult<()> {
        // Withdraw ops counter is set equal to the `verify` ops counter
        // since we assume that we've sent a withdraw for every `verify` op.
        sqlx::query!(
            "UPDATE eth_parameters
            SET commit_ops = $1, verify_ops = $2, withdraw_ops = $3
            WHERE id = true",
            last_committed_block as i64,
            last_verified_block as i64,
            last_verified_block as i64
        )
        .execute(self.0.conn())
        .await?;

        Ok(())
    }

    async fn load_events_state(&mut self, state: &str) -> QueryResult<Vec<StoredBlockEvent>> {
        let events = sqlx::query_as!(
            StoredBlockEvent,
            "SELECT * FROM data_restore_events_state
            WHERE block_type = $1
            ORDER BY block_num ASC",
            state,
        )
        .fetch_all(self.0.conn())
        .await?;

        Ok(events)
    }

    pub async fn load_committed_events_state(&mut self) -> QueryResult<Vec<StoredBlockEvent>> {
        self.load_events_state("Committed").await
    }

    pub async fn load_verified_events_state(&mut self) -> QueryResult<Vec<StoredBlockEvent>> {
        self.load_events_state("Verified").await
    }

    pub async fn load_storage_state(&mut self) -> QueryResult<StoredStorageState> {
        let state = sqlx::query_as!(
            StoredStorageState,
            "SELECT * FROM data_restore_storage_state_update
            LIMIT 1",
        )
        .fetch_one(self.0.conn())
        .await?;

        Ok(state)
    }

    pub(crate) async fn update_storage_state(&mut self, state: NewStorageState) -> QueryResult<()> {
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

        Ok(())
    }

    pub(crate) async fn update_block_events(
        &mut self,
        events: &[NewBlockEvent],
    ) -> QueryResult<()> {
        let mut transaction = self.0.start_transaction().await?;
        sqlx::query!("DELETE FROM data_restore_events_state")
            .execute(transaction.conn())
            .await?;

        for event in events.iter() {
            sqlx::query!(
                "INSERT INTO data_restore_events_state (block_type, transaction_hash, block_num) VALUES ($1, $2, $3)",
                event.block_type, event.transaction_hash, event.block_num
            )
            .execute(transaction.conn())
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }
}
