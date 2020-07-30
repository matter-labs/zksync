// Built-in deps
// External imports
use diesel::prelude::*;
// Workspace imports
use models::{node::BlockNumber, ActionType};
// Local imports
use self::records::{
    NewExecutedPriorityOperation, NewExecutedTransaction, NewOperation,
    StoredExecutedPriorityOperation, StoredExecutedTransaction, StoredOperation,
};
use crate::schema::*;
use crate::{chain::mempool::MempoolSchema, StorageProcessor};

pub mod records;

/// Operations schema is capable of storing and loading the transactions.
/// Every kind of transaction (non-executed, executed, and executed priority tx)
/// can be either saved or loaded from the database.
#[derive(Debug)]
pub struct OperationsSchema<'a>(pub &'a StorageProcessor);

impl<'a> OperationsSchema<'a> {
    pub fn get_operation(
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

    pub fn get_executed_operation(
        &self,
        op_hash: &[u8],
    ) -> QueryResult<Option<StoredExecutedTransaction>> {
        executed_transactions::table
            .filter(executed_transactions::tx_hash.eq(op_hash))
            .first::<StoredExecutedTransaction>(self.0.conn())
            .optional()
    }

    pub fn get_executed_priority_operation(
        &self,
        priority_op_id: u32,
    ) -> QueryResult<Option<StoredExecutedPriorityOperation>> {
        executed_priority_operations::table
            .filter(
                executed_priority_operations::priority_op_serialid.eq(i64::from(priority_op_id)),
            )
            .first::<StoredExecutedPriorityOperation>(self.0.conn())
            .optional()
    }

    /// Loads a range of VERIFY operations. Used when multiblock proof is created.
    pub fn load_verify_operations(
        &self,
        block_range_start: i64,
        block_range_end: i64,
    ) -> QueryResult<Vec<i64>> {
        self.0.conn().transaction(|| {
            let mut operation_ids = Vec::new();
            for block_number in block_range_start..=block_range_end {
                let result: StoredOperation = self
                    .get_operation(block_number as u32, ActionType::VERIFY)
                    .expect("Operation must be created");

                operation_ids.push(result.id);
            }

            Ok(operation_ids)
        })
    }

    pub(crate) fn store_operation(&self, operation: NewOperation) -> QueryResult<StoredOperation> {
        diesel::insert_into(operations::table)
            .values(&operation)
            .get_result(self.0.conn())
    }

    /// Stores the executed operation in the database.
    pub(crate) fn store_executed_operation(
        &self,
        operation: NewExecutedTransaction,
    ) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            MempoolSchema(&self.0).remove_tx(&operation.tx_hash)?;

            if operation.success {
                // If transaction succeed, it should replace the stored tx with the same hash.
                // The situation when a duplicate tx is stored in the database may exist only if has
                // failed previously.
                // Possible scenario: user had no enough funds for transfer, then deposited some and
                // sent the same transfer again.
                diesel::insert_into(executed_transactions::table)
                    .values(&operation)
                    .on_conflict(executed_transactions::tx_hash)
                    .do_update()
                    .set(&operation)
                    .execute(self.0.conn())?;
            } else {
                // If transaction failed, we do nothing on conflict.
                diesel::insert_into(executed_transactions::table)
                    .values(&operation)
                    .on_conflict_do_nothing()
                    .execute(self.0.conn())?;
            };
            Ok(())
        })
    }

    pub(crate) fn store_executed_priority_operation(
        &self,
        operation: NewExecutedPriorityOperation,
    ) -> QueryResult<()> {
        diesel::insert_into(executed_priority_operations::table)
            .values(&operation)
            .on_conflict_do_nothing()
            .execute(self.0.conn())?;
        Ok(())
    }
}
