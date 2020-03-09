// Built-in deps
// External imports
use diesel::dsl::max;
use diesel::prelude::*;
use diesel::sql_types::Text;
// Workspace imports
use models::node::{
    block::{Block, ExecutedOperations},
    AccountId, BlockNumber, Fr, FranklinOp,
};
use models::{fe_from_hex, fe_to_hex, Action, ActionType, Operation};
// Local imports
use self::records::{BlockDetails, StorageBlock};
use crate::interfaces::{
    ethereum::records::StorageETHOperation,
    operations::{
        records::{
            NewExecutedPriorityOperation, NewExecutedTransaction, NewOperation,
            StoredExecutedPriorityOperation, StoredExecutedTransaction, StoredOperation,
        },
        OperationsSchema,
    },
    operations_ext::records::{InsertTx, ReadTx},
    state::StateSchema,
};
use crate::schema::*;
use crate::StorageProcessor;

mod conversion;
pub mod records;

pub struct BlockSchema<'a>(pub &'a StorageProcessor);

impl<'a> From<&'a StorageProcessor> for BlockSchema<'a> {
    fn from(processor: &'a StorageProcessor) -> Self {
        Self(processor)
    }
}

impl<'a> BlockSchema<'a> {
    /// Execute an operation: store op, modify state accordingly, load additional data and meta tx info
    /// - Commit => store account updates
    /// - Verify => apply account updates
    pub fn execute_operation(&self, op: Operation) -> QueryResult<Operation> {
        self.0.conn().transaction(|| {
            let block_number = op.block.block_number;

            match &op.action {
                Action::Commit => {
                    StateSchema(self.0)
                        .commit_state_update(op.block.block_number, &op.accounts_updated)?;
                    self.save_block(op.block)?;
                }
                Action::Verify { .. } => {
                    StateSchema(self.0).apply_state_update(op.block.block_number)?
                }
            };

            let new_operation = NewOperation {
                block_number: i64::from(block_number),
                action_type: op.action.to_string(),
            };
            let stored: StoredOperation =
                OperationsSchema(self.0).store_operation(new_operation)?;
            stored.into_op(self.0)
        })
    }

    pub(crate) fn save_block_transactions(&self, block: Block) -> QueryResult<()> {
        for block_tx in block.block_transactions.into_iter() {
            match block_tx {
                ExecutedOperations::Tx(tx) => {
                    let hash = tx.tx.hash().as_ref().to_vec();
                    let primary_account_address = tx.tx.account().as_bytes().to_vec();
                    let nonce = tx.tx.nonce() as i64;
                    let serialized_tx = serde_json::to_value(&tx.tx).unwrap_or_default();

                    let new_tx = NewExecutedTransaction::prepare_stored_tx(*tx, block.block_number);
                    diesel::insert_into(mempool::table)
                        .values(&InsertTx {
                            hash,
                            primary_account_address,
                            nonce,
                            tx: serialized_tx,
                        })
                        .on_conflict_do_nothing()
                        .execute(self.0.conn())?;
                    OperationsSchema(self.0).store_executed_operation(new_tx)?;
                }
                ExecutedOperations::PriorityOp(prior_op) => {
                    let new_priority_op = NewExecutedPriorityOperation::prepare_stored_priority_op(
                        *prior_op,
                        block.block_number,
                    );
                    OperationsSchema(self.0).store_executed_priority_operation(new_priority_op)?;
                }
            }
        }
        Ok(())
    }

    pub fn get_block(&self, block: BlockNumber) -> QueryResult<Option<Block>> {
        let stored_block = if let Some(block) = blocks::table
            .find(i64::from(block))
            .first::<StorageBlock>(self.0.conn())
            .optional()?
        {
            block
        } else {
            return Ok(None);
        };

        let block_transactions = self.get_block_executed_ops(block)?;

        assert!(stored_block.root_hash.starts_with("sync-bl:"));
        let new_root_hash = fe_from_hex::<Fr>(&format!("0x{}", &stored_block.root_hash[8..]))
            .expect("Unparsable root hash");

        Ok(Some(Block {
            block_number: block,
            new_root_hash,
            fee_account: stored_block.fee_account_id as AccountId,
            block_transactions,
            processed_priority_ops: (
                stored_block.unprocessed_prior_op_before as u64,
                stored_block.unprocessed_prior_op_after as u64,
            ),
        }))
    }

    pub fn get_block_operations(&self, block: BlockNumber) -> QueryResult<Vec<FranklinOp>> {
        let executed_ops = self.get_block_executed_ops(block)?;
        Ok(executed_ops
            .into_iter()
            .filter_map(|exec_op| match exec_op {
                ExecutedOperations::Tx(tx) => tx.op,
                ExecutedOperations::PriorityOp(priorop) => Some(priorop.op),
            })
            .collect())
    }

    pub fn get_block_executed_ops(
        &self,
        block: BlockNumber,
    ) -> QueryResult<Vec<ExecutedOperations>> {
        self.0.conn().transaction(|| {
            let mut executed_operations = Vec::new();

            let stored_executed_txs: Vec<_> = executed_transactions::table
                .left_join(mempool::table.on(executed_transactions::tx_hash.eq(mempool::hash)))
                .filter(executed_transactions::block_number.eq(i64::from(block)))
                .load::<(StoredExecutedTransaction, Option<ReadTx>)>(self.0.conn())?;
            let executed_txs = stored_executed_txs
                .into_iter()
                .filter_map(|(stored_exec, stored_tx)| stored_exec.into_executed_tx(stored_tx).ok())
                .map(|tx| ExecutedOperations::Tx(Box::new(tx)));
            executed_operations.extend(executed_txs);

            let stored_executed_prior_ops: Vec<_> = executed_priority_operations::table
                .filter(executed_priority_operations::block_number.eq(i64::from(block)))
                .load::<StoredExecutedPriorityOperation>(self.0.conn())?;
            let executed_prior_ops = stored_executed_prior_ops
                .into_iter()
                .map(|op| ExecutedOperations::PriorityOp(Box::new(op.into_executed())));
            executed_operations.extend(executed_prior_ops);

            executed_operations.sort_by_key(|exec_op| {
                match exec_op {
                    ExecutedOperations::Tx(tx) => {
                        if let Some(idx) = tx.block_index {
                            idx
                        } else {
                            // failed operations are at the end.
                            u32::max_value()
                        }
                    }
                    ExecutedOperations::PriorityOp(op) => op.block_index,
                }
            });

            Ok(executed_operations)
        })
    }

    pub fn load_stored_op_with_block_number(
        &self,
        block_number: BlockNumber,
        action_type: ActionType,
    ) -> Option<StoredOperation> {
        use crate::schema::operations::dsl;
        dsl::operations
            .filter(dsl::block_number.eq(i64::from(block_number)))
            .filter(dsl::action_type.eq(action_type.to_string().as_str()))
            .get_result(self.0.conn())
            .ok()
    }

    pub fn load_block_range(
        &self,
        max_block: BlockNumber,
        limit: u32,
    ) -> QueryResult<Vec<BlockDetails>> {
        let query = format!(
            "
            with eth_ops as (
                select
                    operations.block_number,
                    '0x' || encode(eth_operations.tx_hash::bytea, 'hex') as tx_hash,
                    operations.action_type,
                    operations.created_at
                from operations
                    left join eth_operations on eth_operations.op_id = operations.id
            )
            select
                blocks.number as block_number,
                blocks.root_hash as new_state_root,
                blocks.block_size as block_size,
                commited.tx_hash as commit_tx_hash,
                verified.tx_hash as verify_tx_hash,
                commited.created_at as committed_at,
                verified.created_at as verified_at
            from blocks
            inner join eth_ops commited on
                commited.block_number = blocks.number and commited.action_type = 'COMMIT'
            left join eth_ops verified on
                verified.block_number = blocks.number and verified.action_type = 'VERIFY'
            where
                blocks.number <= {max_block}
            order by blocks.number desc
            limit {limit};
        ",
            max_block = i64::from(max_block),
            limit = i64::from(limit)
        );
        diesel::sql_query(query).load(self.0.conn())
    }

    pub fn handle_search(&self, query: String) -> Option<BlockDetails> {
        let block_number = query.parse::<i64>().unwrap_or(i64::max_value());
        let l_query = query.to_lowercase();
        let sql_query = format!(
            "
            with eth_ops as (
                select
                    operations.block_number,
                    'sync-tx:' || encode(eth_operations.tx_hash::bytea, 'hex') as tx_hash,
                    operations.action_type,
                    operations.created_at
                from operations
                    left join eth_operations on eth_operations.op_id = operations.id
            )
            select
                blocks.number as block_number,
                blocks.root_hash as new_state_root,
                commited.tx_hash as commit_tx_hash,
                verified.tx_hash as verify_tx_hash,
                commited.created_at as committed_at,
                verified.created_at as verified_at
            from blocks
            inner join eth_ops commited on
                commited.block_number = blocks.number and commited.action_type = 'COMMIT'
            left join eth_ops verified on
                verified.block_number = blocks.number and verified.action_type = 'VERIFY'
            where false
                or lower(commited.tx_hash) = $1
                or lower(verified.tx_hash) = $1
                or lower(blocks.root_hash) = $1
                or blocks.number = {block_number}
            order by blocks.number desc
            limit 1;
        ",
            block_number = block_number
        );
        diesel::sql_query(sql_query)
            .bind::<Text, _>(l_query)
            .get_result(self.0.conn())
            .ok()
    }

    pub fn load_commit_op(&self, block_number: BlockNumber) -> Option<Operation> {
        let op = self.load_stored_op_with_block_number(block_number, ActionType::COMMIT);
        op.and_then(|r| r.into_op(self.0).ok())
    }

    pub fn load_committed_block(&self, block_number: BlockNumber) -> Option<Block> {
        self.load_commit_op(block_number).map(|r| r.block)
    }

    pub fn load_unsent_ops(&self) -> QueryResult<Vec<Operation>> {
        self.0.conn().transaction(|| {
            let ops: Vec<_> = operations::table
                .left_join(eth_operations::table.on(eth_operations::op_id.eq(operations::id)))
                .filter(eth_operations::id.is_null())
                .order(operations::id.asc())
                .load::<(StoredOperation, Option<StorageETHOperation>)>(self.0.conn())?;
            ops.into_iter().map(|(o, _)| o.into_op(self.0)).collect()
        })
    }

    pub fn load_unverified_commits_after_block(
        &self,
        block_size: usize,
        block: BlockNumber,
        limit: i64,
    ) -> QueryResult<Vec<Operation>> {
        self.0.conn().transaction(|| {
            let ops: Vec<StoredOperation> = diesel::sql_query(format!(
                "
                WITH sized_operations AS (
                    SELECT operations.* FROM operations
                    LEFT JOIN blocks ON number = block_number
                    LEFT JOIN proofs USING (block_number)
                    WHERE proof IS NULL AND block_size = {}
                )
                SELECT * FROM sized_operations
                WHERE action_type = 'COMMIT'
                    AND block_number > (
                        SELECT COALESCE(max(block_number), 0)
                        FROM sized_operations
                        WHERE action_type = 'VERIFY'
                    )
                    AND block_number > {}
                ORDER BY block_number
                LIMIT {}
                ",
                block_size, block, limit
            ))
            .load(self.0.conn())?;
            ops.into_iter().map(|o| o.into_op(self.0)).collect()
        })
    }

    pub fn get_last_committed_block(&self) -> QueryResult<BlockNumber> {
        use crate::schema::operations::dsl::*;
        operations
            .select(max(block_number))
            .filter(action_type.eq(&ActionType::COMMIT.to_string()))
            .get_result::<Option<i64>>(self.0.conn())
            .map(|max| max.unwrap_or(0) as BlockNumber)
    }

    pub fn get_last_verified_block(&self) -> QueryResult<BlockNumber> {
        use crate::schema::operations::dsl::*;
        operations
            .select(max(block_number))
            .filter(action_type.eq(&ActionType::VERIFY.to_string()))
            .get_result::<Option<i64>>(self.0.conn())
            .map(|max| max.unwrap_or(0) as BlockNumber)
    }

    pub(crate) fn save_block(&self, block: Block) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            let number = i64::from(block.block_number);
            let root_hash = format!("sync-bl:{}", fe_to_hex(&block.new_root_hash));
            let fee_account_id = i64::from(block.fee_account);
            let unprocessed_prior_op_before = block.processed_priority_ops.0 as i64;
            let unprocessed_prior_op_after = block.processed_priority_ops.1 as i64;
            let block_size = block.smallest_block_size() as i64;

            self.save_block_transactions(block)?;

            let new_block = StorageBlock {
                number,
                root_hash,
                fee_account_id,
                unprocessed_prior_op_before,
                unprocessed_prior_op_after,
                block_size,
            };

            diesel::insert_into(blocks::table)
                .values(&new_block)
                .execute(self.0.conn())?;

            Ok(())
        })
    }
}
