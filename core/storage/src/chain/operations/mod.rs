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
use crate::StorageProcessor;

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

    pub(crate) fn store_operation(&self, operation: NewOperation) -> QueryResult<StoredOperation> {
        diesel::insert_into(operations::table)
            .values(&operation)
            .get_result(self.0.conn())
    }

    /// Stores the executed operation in the database.
    pub(crate) fn store_executed_operation(
        &self,
        operation: NewExecutedTransaction,
    ) -> QueryResult<StoredExecutedTransaction> {
        diesel::insert_into(executed_transactions::table)
            .values(&operation)
            .get_result(self.0.conn())
    }

    pub(crate) fn store_executed_priority_operation(
        &self,
        operation: NewExecutedPriorityOperation,
    ) -> QueryResult<StoredExecutedPriorityOperation> {
        diesel::insert_into(executed_priority_operations::table)
            .values(&operation)
            .get_result(self.0.conn())
    }
}
