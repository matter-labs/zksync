// Built-in deps
// External imports
use diesel::prelude::*;
use itertools::Itertools;
// Workspace imports
use models::node::block::Block;
use models::node::{AccountId, AccountUpdate, BlockNumber, FranklinOp};
use models::{Operation, TokenAddedEvent};
// Local imports
use self::records::StoredRollupOpsBlock;
use crate::schema::*;
use crate::StorageProcessor;
use crate::{
    chain::{
        block::BlockSchema,
        operations::records::{NewFranklinOp, StoredFranklinOp},
        state::records::{NewBlockEvent, NewStorageState},
        state::StateSchema,
    },
    ethereum::{records::NewLastWatchedEthBlockNumber, EthereumSchema},
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
            StateSchema(self.0).update_storage_state(self.new_storage_state("None"))?;
            Ok(())
        })
    }

    pub fn save_block_operations(
        &self,
        commit_op: Operation,
        verify_op: Operation,
    ) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            BlockSchema(self.0).execute_operation(commit_op)?;
            BlockSchema(self.0).execute_operation(verify_op)?;
            StateSchema(self.0).update_storage_state(self.new_storage_state("None"))?;
            Ok(())
        })
    }

    pub fn save_events_state(
        &self,
        block_events: &[NewBlockEvent],
        token_events: &[TokenAddedEvent],
        last_watched_eth_number: &NewLastWatchedEthBlockNumber,
    ) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            StateSchema(self.0).update_block_events(block_events)?;

            for token in token_events.iter() {
                TokensSchema(self.0).store_token(
                    token.id as u16,
                    &format!("0x{:x}", token.address),
                    &format!("ERC20-{}", token.id),
                )?;
            }

            EthereumSchema(self.0).update_last_watched_block_number(last_watched_eth_number)?;
            StateSchema(self.0).update_storage_state(self.new_storage_state("Events"))?;

            Ok(())
        })
    }

    pub fn save_rollup_ops(
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
            StateSchema(self.0).update_storage_state(self.new_storage_state("Operations"))?;
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

    fn new_storage_state(&self, state: impl ToString) -> NewStorageState {
        NewStorageState {
            storage_state: state.to_string(),
        }
    }
}
