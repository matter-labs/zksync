// Built-in deps
// External imports
use diesel::prelude::*;
use itertools::Itertools;
// Workspace imports
use models::node::block::Block;
use models::node::{AccountId, AccountUpdate, BlockNumber, FranklinOp};
use models::{Action, Operation, TokenAddedEvent};
// Local imports
use self::records::StoredRollupOpsBlock;
use crate::interfaces::{
    block::BlockSchema,
    ethereum::records::NewLastWatchedEthBlockNumber,
    operations::records::{NewFranklinOp, NewOperation, StoredFranklinOp, StoredOperation},
    state::records::{NewBlockEvent, NewStorageState},
    state::StateSchema,
    tokens::TokensSchema,
};
use crate::schema::*;
use crate::StorageProcessor;

pub mod records;

pub struct DataRestoreSchema<'a>(pub &'a StorageProcessor);

impl<'a> DataRestoreSchema<'a> {
    pub fn save_block_transactions_with_data_restore_state(&self, block: Block) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            BlockSchema(self.0).save_block_transactions(block)?;
            let state = NewStorageState {
                storage_state: "None".to_string(),
            };
            self.update_storage_state(state)?;
            Ok(())
        })
    }

    pub fn save_block_operations_with_data_restore_state(
        &self,
        commit_op: Operation,
        verify_op: Operation,
    ) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            self.save_operation(commit_op)?;
            self.save_operation(verify_op)?;
            let state = NewStorageState {
                storage_state: "None".to_string(),
            };
            self.update_storage_state(state)?;
            Ok(())
        })
    }

    pub fn save_events_state_with_data_restore_state(
        &self,
        block_events: &[NewBlockEvent],
        token_events: &[TokenAddedEvent],
        last_watched_eth_number: &NewLastWatchedEthBlockNumber,
    ) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            self.update_block_events(block_events)?;

            for token in token_events.iter() {
                TokensSchema(self.0).store_token(
                    token.id as u16,
                    &format!("0x{:x}", token.address),
                    &format!("ERC20-{}", token.id),
                )?;
            }

            self.update_last_watched_block_number(last_watched_eth_number)?;

            let state = NewStorageState {
                storage_state: "Events".to_string(),
            };
            self.update_storage_state(state)?;

            Ok(())
        })
    }

    pub fn save_rollup_ops_with_data_restore_state(
        &self,
        ops: &[(BlockNumber, &FranklinOp, AccountId)],
    ) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            diesel::delete(rollup_ops::table).execute(self.0.conn())?;
            for op in ops.iter() {
                let stored_op = NewFranklinOp::prepare_stored_op(&op.1, op.0, op.2);
                diesel::insert_into(rollup_ops::table)
                    .values(&stored_op)
                    .execute(self.0.conn())?;
            }
            let state = NewStorageState {
                storage_state: "Operations".to_string(),
            };
            self.update_storage_state(state)?;
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
        let stored_operations = rollup_ops::table
            .order(rollup_ops::id.asc())
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

    fn save_operation(&self, op: Operation) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            let block_number = i64::from(op.block.block_number);
            let action_type = op.action.to_string();

            match &op.action {
                Action::Commit => {
                    StateSchema(self.0)
                        .commit_state_update(op.block.block_number, &op.accounts_updated)?;
                    BlockSchema(self.0).save_block(op.block)?;
                }
                Action::Verify { .. } => {
                    StateSchema(self.0).apply_state_update(op.block.block_number)?
                }
            };

            let _stored: StoredOperation = diesel::insert_into(operations::table)
                .values(&NewOperation {
                    block_number,
                    action_type,
                })
                .get_result(self.0.conn())?;
            Ok(())
        })
    }

    fn update_block_events(&self, events: &[NewBlockEvent]) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            diesel::delete(events_state::table).execute(self.0.conn())?;
            for event in events.iter() {
                diesel::insert_into(events_state::table)
                    .values(event)
                    .execute(self.0.conn())?;
            }
            Ok(())
        })
    }

    fn update_last_watched_block_number(
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

    fn update_storage_state(&self, state: NewStorageState) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            diesel::delete(storage_state_update::table).execute(self.0.conn())?;
            diesel::insert_into(storage_state_update::table)
                .values(state)
                .execute(self.0.conn())?;
            Ok(())
        })
    }
}
