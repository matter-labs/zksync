// Built-in deps
// External imports
use diesel::prelude::*;
// Workspace imports
// Local imports
use self::records::{
    NewExecutedPriorityOperation, NewExecutedTransaction, NewOperation,
    StoredExecutedPriorityOperation, StoredExecutedTransaction, StoredOperation,
};
use crate::schema::*;
use crate::StorageProcessor;

pub mod records;

pub struct OperationsSchema<'a>(pub &'a StorageProcessor);

impl<'a> OperationsSchema<'a> {
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
