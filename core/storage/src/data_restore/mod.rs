// Built-in deps
// External imports
use diesel::dsl::update;
use diesel::prelude::*;
use itertools::Itertools;
// Workspace imports
use models::node::block::Block;
use models::node::{AccountId, AccountUpdate, BlockNumber, FranklinOp, Token};
use models::{Operation, TokenAddedEvent};
// Local imports
use self::records::{
    NewBlockEvent, NewFranklinOp, NewLastWatchedEthBlockNumber, NewStorageState, StoredBlockEvent,
    StoredFranklinOp, StoredLastWatchedEthBlockNumber, StoredRollupOpsBlock, StoredStorageState,
};
use crate::schema::*;
use crate::StorageProcessor;
use crate::{
    chain::{block::BlockSchema, state::StateSchema},
    tokens::TokensSchema,
};

pub mod records;

/// Data restore schema provides a convenient interface to restore the
/// sidechain state from the Ethereum contract.
///
/// This schema is used exclusively by the `data_restore` crate.
#[derive(Debug)]
pub struct DataRestoreSchema<'a>(pub &'a StorageProcessor);

impl<'a> DataRestoreSchema<'a> {
    pub fn save_block_transactions(&self, block: Block) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            BlockSchema(self.0).save_block_transactions(block)?;
            self.update_storage_state(self.new_storage_state("None"))?;
            Ok(())
        })
    }

    pub fn save_block_operations(
        &self,
        commit_op: Operation,
        verify_op: Operation,
    ) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            let commit_op = BlockSchema(self.0).execute_operation(commit_op)?;
            let verify_op = BlockSchema(self.0).execute_operation(verify_op)?;
            update(
                operations::table.filter(
                    operations::id.eq_any(vec![commit_op.id.unwrap(), verify_op.id.unwrap()]),
                ),
            )
            .set(operations::confirmed.eq(true))
            .execute(self.0.conn())
            .map(drop)?;
            self.update_storage_state(self.new_storage_state("None"))?;
            Ok(())
        })
    }

    pub fn save_genesis_state(&self, genesis_acc_update: AccountUpdate) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            StateSchema(self.0).commit_state_update(0, &[(0, genesis_acc_update)])?;
            StateSchema(self.0).apply_state_update(0)?;
            Ok(())
        })
    }

    pub fn load_rollup_ops_blocks(&self) -> QueryResult<Vec<StoredRollupOpsBlock>> {
        let stored_operations = data_restore_rollup_ops::table
            .order(data_restore_rollup_ops::id.asc())
            .load::<StoredFranklinOp>(self.0.conn())?;
        let ops_blocks: Vec<StoredRollupOpsBlock> = stored_operations
            .into_iter()
            .group_by(|op| op.block_num)
            .into_iter()
            .map(|(_, stored_ops)| {
                // let stored_ops = group.clone();
                // let mut ops: Vec<FranklinOp> = vec![];
                let mut block_num: i64 = 0;
                let mut fee_account: i64 = 0;
                let ops: Vec<FranklinOp> = stored_ops
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
    pub(crate) fn update_last_watched_block_number(
        &self,
        number: &NewLastWatchedEthBlockNumber,
    ) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            diesel::delete(data_restore_last_watched_eth_block::table).execute(self.0.conn())?;
            diesel::insert_into(data_restore_last_watched_eth_block::table)
                .values(number)
                .execute(self.0.conn())?;
            Ok(())
        })
    }

    /// Loads the last seen Ethereum block number.
    pub fn load_last_watched_block_number(&self) -> QueryResult<StoredLastWatchedEthBlockNumber> {
        data_restore_last_watched_eth_block::table.first(self.0.conn())
    }

    fn new_storage_state(&self, state: impl ToString) -> NewStorageState {
        NewStorageState {
            storage_state: state.to_string(),
        }
    }

    pub fn save_events_state(
        &self,
        block_events: &[NewBlockEvent],
        token_events: &[TokenAddedEvent],
        last_watched_eth_number: &NewLastWatchedEthBlockNumber,
    ) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            self.update_block_events(block_events)?;

            for &TokenAddedEvent { id, address } in token_events.iter() {
                let token = Token::new(id, address, &format!("ERC20-{}", id));
                TokensSchema(self.0).store_token(token)?;
            }

            self.update_last_watched_block_number(last_watched_eth_number)?;
            self.update_storage_state(self.new_storage_state("Events"))?;

            Ok(())
        })
    }

    pub fn save_rollup_ops(
        &self,
        ops: &[(BlockNumber, &FranklinOp, AccountId)],
    ) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            diesel::delete(data_restore_rollup_ops::table).execute(self.0.conn())?;
            for op in ops.iter() {
                let stored_op = NewFranklinOp::prepare_stored_op(&op.1, op.0, op.2);
                diesel::insert_into(data_restore_rollup_ops::table)
                    .values(&stored_op)
                    .execute(self.0.conn())?;
            }
            self.update_storage_state(self.new_storage_state("Operations"))?;
            Ok(())
        })
    }

    /// Method that initializes the `eth_stats` table.
    /// Since `eth_sender` module uses this table to identify the expected next block numbers
    /// for sending operations to the Ethereum, we must initialize it with actual values.
    pub fn initialize_eth_stats(
        &self,
        last_committed_block: BlockNumber,
        last_verified_block: BlockNumber,
    ) -> QueryResult<()> {
        // Withdraw ops counter is set equal to the `verify` ops counter
        // since we assume that we've sent a withdraw for every `verify` op.
        update(eth_stats::table.filter(eth_stats::id.eq(true)))
            .set((
                eth_stats::commit_ops.eq(last_committed_block as i64),
                eth_stats::verify_ops.eq(last_verified_block as i64),
                eth_stats::withdraw_ops.eq(last_verified_block as i64),
            ))
            .execute(self.0.conn())?;

        Ok(())
    }

    pub fn load_committed_events_state(&self) -> QueryResult<Vec<StoredBlockEvent>> {
        let events = data_restore_events_state::table
            .filter(data_restore_events_state::block_type.eq("Committed".to_string()))
            .order(data_restore_events_state::block_num.asc())
            .load::<StoredBlockEvent>(self.0.conn())?;
        Ok(events)
    }

    pub fn load_verified_events_state(&self) -> QueryResult<Vec<StoredBlockEvent>> {
        let events = data_restore_events_state::table
            .filter(data_restore_events_state::block_type.eq("Verified".to_string()))
            .order(data_restore_events_state::block_num.asc())
            .load::<StoredBlockEvent>(self.0.conn())?;
        Ok(events)
    }

    pub fn load_storage_state(&self) -> QueryResult<StoredStorageState> {
        data_restore_storage_state_update::table.first(self.0.conn())
    }

    pub(crate) fn update_storage_state(&self, state: NewStorageState) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            diesel::delete(data_restore_storage_state_update::table).execute(self.0.conn())?;
            diesel::insert_into(data_restore_storage_state_update::table)
                .values(state)
                .execute(self.0.conn())?;
            Ok(())
        })
    }

    pub(crate) fn update_block_events(&self, events: &[NewBlockEvent]) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            diesel::delete(data_restore_events_state::table).execute(self.0.conn())?;
            for event in events.iter() {
                diesel::insert_into(data_restore_events_state::table)
                    .values(event)
                    .execute(self.0.conn())?;
            }
            Ok(())
        })
    }
}
