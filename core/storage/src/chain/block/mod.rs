// Built-in deps
// External imports
use diesel::dsl::max;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
// Workspace imports
use models::node::{
    block::{Block, ExecutedOperations},
    AccountId, BlockNumber, FranklinOp,
};
use models::{fe_from_bytes, fe_to_bytes, Action, ActionType, Operation};
// Local imports
use self::records::{BlockDetails, StorageBlock};
use crate::prover::records::StoredProof;
use crate::schema::*;
use crate::StorageProcessor;
use crate::{
    chain::{
        operations::{
            records::{
                NewExecutedPriorityOperation, NewExecutedTransaction, NewOperation,
                StoredExecutedPriorityOperation, StoredExecutedTransaction, StoredOperation,
            },
            OperationsSchema,
        },
        state::StateSchema,
    },
    ethereum::records::ETHBinding,
};

mod conversion;
pub mod records;

/// Block schema is a primary sidechain storage controller.
///
/// Besides block getters/setters, it provides an `execute_operation` method,
/// which is essential for the sidechain logic, as it causes the state updates in the chain.
#[derive(Debug)]
pub struct BlockSchema<'a>(pub &'a StorageProcessor);

impl<'a> BlockSchema<'a> {
    /// Executes an operation:
    /// 1. Store the operation.
    /// 2. Modify the state according to the operation changes:
    ///   - Commit => store account updates.
    ///   - Verify => apply account updates.
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

    /// Given a block, stores its transactions in the database.
    pub fn save_block_transactions(&self, block: Block) -> QueryResult<()> {
        for block_tx in block.block_transactions.into_iter() {
            match block_tx {
                ExecutedOperations::Tx(tx) => {
                    // Store the executed operation in the corresponding schema.
                    let new_tx = NewExecutedTransaction::prepare_stored_tx(*tx, block.block_number);
                    OperationsSchema(self.0).store_executed_operation(new_tx)?;
                }
                ExecutedOperations::PriorityOp(prior_op) => {
                    // For priority operation we should only store it in the Operations schema.
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

    /// Given the block number, attempts to retrieve it from the database.
    /// Returns `None` if the block with provided number does not exist yet.
    pub fn get_block(&self, block: BlockNumber) -> QueryResult<Option<Block>> {
        // Load block header.
        let stored_block = if let Some(block) = blocks::table
            .find(i64::from(block))
            .first::<StorageBlock>(self.0.conn())
            .optional()?
        {
            block
        } else {
            return Ok(None);
        };

        // Load transactions for this block.
        let block_transactions = self.get_block_executed_ops(block)?;

        // Encode the root hash as `0xFF..FF`.
        let new_root_hash = fe_from_bytes(&stored_block.root_hash).expect("Unparsable root hash");

        // Return the obtained block in the expected format.
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

    /// Same as `get_block_executed_ops`, but returns a vector of `FranklinOp` instead
    /// of `ExecutedOperations`.
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

    /// Given the block number, loads all the operations that were executed in that block.
    pub fn get_block_executed_ops(
        &self,
        block: BlockNumber,
    ) -> QueryResult<Vec<ExecutedOperations>> {
        let mut executed_operations = Vec::new();

        // Load both executed transactions and executed priority operations
        // from the database.
        let (executed_ops, executed_priority_ops) =
            self.0.conn().transaction::<_, DieselError, _>(|| {
                let executed_ops = executed_transactions::table
                    .filter(executed_transactions::block_number.eq(i64::from(block)))
                    .load::<StoredExecutedTransaction>(self.0.conn())?;

                let executed_priority_ops = executed_priority_operations::table
                    .filter(executed_priority_operations::block_number.eq(i64::from(block)))
                    .load::<StoredExecutedPriorityOperation>(self.0.conn())?;

                Ok((executed_ops, executed_priority_ops))
            })?;

        // Transform executed operations to be `ExecutedOperations`.
        let executed_ops = executed_ops
            .into_iter()
            .filter_map(|stored_exec| stored_exec.into_executed_tx().ok())
            .map(|tx| ExecutedOperations::Tx(Box::new(tx)));
        executed_operations.extend(executed_ops);

        // Transform executed priority operations to be `ExecutedOperations`.
        let executed_priority_ops = executed_priority_ops
            .into_iter()
            .map(|op| ExecutedOperations::PriorityOp(Box::new(op.into_executed())));
        executed_operations.extend(executed_priority_ops);

        // Sort the operations, so all the failed operations will be at the very end
        // of the list.
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
    }

    /// Loads the block headers for the given amount of blocks.
    pub fn load_block_range(
        &self,
        max_block: BlockNumber,
        limit: u32,
    ) -> QueryResult<Vec<BlockDetails>> {
        // This query does the following:
        // - joins the `operations` and `eth_tx_hashes` (using the intermediate `eth_ops_binding` table)
        //   tables to collect the data:
        //   block number, ethereum transaction hash, action type and action creation timestamp;
        // - joins the `blocks` table with result of the join twice: once for committed operations
        //   and verified operations;
        // - collects the {limit} blocks in the descending order with the data gathered above.
        let query = format!(
            " \
            with eth_ops as ( \
                select \
                    operations.block_number, \
                    eth_tx_hashes.tx_hash, \
                    operations.action_type, \
                    operations.created_at \
                from operations \
                    left join eth_ops_binding on eth_ops_binding.op_id = operations.id \
                    left join eth_tx_hashes on eth_tx_hashes.eth_op_id = eth_ops_binding.eth_op_id \
            ) \
            select \
                DISTINCT blocks.number as block_number, \
                blocks.root_hash as new_state_root, \
                blocks.block_size as block_size, \
                committed.tx_hash as commit_tx_hash, \
                verified.tx_hash as verify_tx_hash, \
                committed.created_at as committed_at, \
                verified.created_at as verified_at \
            from blocks \
            inner join eth_ops committed on \
                committed.block_number = blocks.number and committed.action_type = 'COMMIT' \
            left join eth_ops verified on \
                verified.block_number = blocks.number and verified.action_type = 'VERIFY' \
            where \
                blocks.number <= {max_block} \
            order by blocks.number desc \
            limit {limit}; \
            ",
            max_block = i64::from(max_block),
            limit = i64::from(limit)
        );
        diesel::sql_query(query).load(self.0.conn())
    }

    /// Helper method for `find_block_by_height_or_hash`. It checks whether
    /// provided string can be interpreted like a hash, and if so, returns the
    /// hexadecimal string without prefix.
    fn try_parse_hex(&self, query: &str) -> Option<String> {
        const HASH_STRING_SIZE: usize = 32 * 2; // 32 bytes, 2 symbols per byte.

        if query.starts_with("0x") {
            Some(query[2..].into())
        } else if query.starts_with("sync-bl:") {
            Some(query[8..].into())
        } else if query.len() == HASH_STRING_SIZE && hex::decode(query).is_ok() {
            Some(query.into())
        } else {
            None
        }
    }

    /// Performs a database search with an uncertain query, which can be either of:
    /// - Hash of commit/verify Ethereum transaction for the block.
    /// - The state root hash of the block.
    /// - The number of the block.
    ///
    /// Will return `None` if the query is malformed or there is no block that matches
    /// the query.
    pub fn find_block_by_height_or_hash(&self, query: String) -> Option<BlockDetails> {
        // Adapt the SQL query based on the input data format.
        let mut where_condition = String::new();

        // If the input looks like hash, add the hash lookup part.
        if let Some(hex_query) = self.try_parse_hex(&query) {
            let hash_lookup = format!(
                " \
                or committed.tx_hash = decode('{hex_query}', 'hex') \
                or verified.tx_hash = decode('{hex_query}', 'hex') \
                or blocks.root_hash = decode('{hex_query}', 'hex') \
                ",
                hex_query = hex_query
            );

            where_condition += &hash_lookup;
        };

        // If the input can be interpreted as integer, add the block number lookup part.
        if let Ok(int_query) = query.parse::<i64>() {
            let block_lookup = format!("or blocks.number = {}", int_query);

            where_condition += &block_lookup;
        }

        // If `where` condition is empty (input doesn't look like hash or integer), no query
        // should be performed.
        if where_condition.is_empty() {
            return None;
        }

        // This query does the following:
        // - joins the `operations` and `eth_tx_hashes` (using the intermediate `eth_ops_binding` table)
        //   tables to collect the data:
        //   block number, ethereum transaction hash, action type and action creation timestamp;
        // - joins the `blocks` table with result of the join twice: once for committed operations
        //   and verified operations;
        // - takes the only block that satisfies one of the following criteria
        //   + query equals to the ETH commit transaction hash (in form of `0x00{..}00`);
        //   + query equals to the ETH verify transaction hash (in form of `0x00{..}00`);
        //   + query equals to the state hash obtained in the block (in form of `sync-bl:00{..}00`);
        //   + query equals to the number of the block.
        let sql_query = format!(
            " \
            with eth_ops as ( \
                select \
                    operations.block_number, \
                    eth_tx_hashes.tx_hash, \
                    operations.action_type, \
                    operations.created_at \
                from operations \
                    left join eth_ops_binding on eth_ops_binding.op_id = operations.id \
                    left join eth_tx_hashes on eth_tx_hashes.eth_op_id = eth_ops_binding.eth_op_id \
            ) \
            select \
                blocks.number as block_number, \
                blocks.root_hash as new_state_root, \
                blocks.block_size as block_size, \
                committed.tx_hash as commit_tx_hash, \
                verified.tx_hash as verify_tx_hash, \
                committed.created_at as committed_at, \
                verified.created_at as verified_at \
            from blocks \
            inner join eth_ops committed on \
                committed.block_number = blocks.number and committed.action_type = 'COMMIT' \
            left join eth_ops verified on \
                verified.block_number = blocks.number and verified.action_type = 'VERIFY' \
            where false \
                {where_condition} \
            order by blocks.number desc \
            limit 1; \
            ",
            where_condition = where_condition
        );

        diesel::sql_query(sql_query).get_result(self.0.conn()).ok()
    }

    pub fn load_commit_op(&self, block_number: BlockNumber) -> Option<Operation> {
        let op = OperationsSchema(self.0).get_operation(block_number, ActionType::COMMIT);
        op.and_then(|r| r.into_op(self.0).ok())
    }

    pub fn load_committed_block(&self, block_number: BlockNumber) -> Option<Block> {
        self.load_commit_op(block_number).map(|r| r.block)
    }

    pub fn load_unsent_ops(&self) -> QueryResult<Vec<Operation>> {
        self.0.conn().transaction(|| {
            let ops: Vec<_> = operations::table
                .left_join(eth_ops_binding::table.on(eth_ops_binding::op_id.eq(operations::id)))
                .filter(eth_ops_binding::id.is_null())
                .order(operations::id.asc())
                .load::<(StoredOperation, Option<ETHBinding>)>(self.0.conn())?;
            ops.into_iter().map(|(o, _)| o.into_op(self.0)).collect()
        })
    }

    /// Returns tuple (commit operation, true if there is proof for the operation).
    pub fn load_commits_after_block(
        &self,
        block: BlockNumber,
        limit: i64,
    ) -> QueryResult<Vec<(Operation, bool)>> {
        self.0.conn().transaction(|| {
            let ops: Vec<(StoredOperation, Option<StoredProof>)> = diesel::sql_query(format!(
                "
                WITH sized_operations AS (
                    SELECT operations.*, proofs.*, operations.block_number as the_block_number
                      FROM operations
                           LEFT JOIN blocks
                                  ON number = block_number
                           LEFT JOIN proofs
                               USING (block_number)
                )
                SELECT *
                  FROM sized_operations
                 WHERE action_type = 'COMMIT'
                   AND the_block_number > (
                        SELECT COALESCE(max(the_block_number), 0)
                          FROM sized_operations
                         WHERE action_type = 'VERIFY'
                    )
                   AND the_block_number > {}
                ORDER BY the_block_number
                LIMIT {}
                ",
                block, limit
            ))
            .load(self.0.conn())?;
            ops.into_iter()
                .map(|(o, p)| {
                    let op = o.into_op(self.0)?;
                    Ok((op, p.is_some()))
                })
                .collect()
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
            let root_hash = fe_to_bytes(&block.new_root_hash);
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
