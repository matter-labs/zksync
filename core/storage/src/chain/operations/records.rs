// External imports
use chrono::prelude::*;
use serde_json::value::Value;
// Workspace imports
use models::node::{AccountId, BlockNumber, FranklinOp};
// Local imports
use crate::schema::*;

#[derive(Debug, Clone, Insertable)]
#[table_name = "executed_priority_operations"]
pub struct NewExecutedPriorityOperation {
    pub block_number: i64,
    pub block_index: i32,
    pub operation: Value,
    pub priority_op_serialid: i64,
    pub deadline_block: i64,
    pub eth_hash: Vec<u8>,
}

#[derive(Debug, Clone, Insertable)]
#[table_name = "operations"]
pub struct NewOperation {
    pub block_number: i64,
    pub action_type: String,
}

#[derive(Debug, Clone, Queryable, QueryableByName)]
#[table_name = "operations"]
pub struct StoredOperation {
    pub id: i64,
    pub block_number: i64,
    pub action_type: String,
    pub created_at: NaiveDateTime,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Queryable, QueryableByName)]
#[table_name = "executed_priority_operations"]
pub struct StoredExecutedPriorityOperation {
    pub id: i32,
    pub block_number: i64,
    pub block_index: i32,
    pub operation: Value,
    pub priority_op_serialid: i64,
    pub deadline_block: i64,
    pub eth_hash: Vec<u8>,
}

#[derive(Debug, Clone, Queryable, QueryableByName)]
#[table_name = "rollup_ops"]
pub struct StoredFranklinOp {
    pub id: i32,
    pub block_num: i64,
    pub operation: Value,
    pub fee_account: i64,
}

impl StoredFranklinOp {
    pub fn into_franklin_op(self) -> FranklinOp {
        serde_json::from_value(self.operation).expect("Unparsable FranklinOp in db")
    }
}
#[derive(Debug, Clone, Insertable)]
#[table_name = "rollup_ops"]
pub struct NewFranklinOp {
    pub block_num: i64,
    pub operation: Value,
    pub fee_account: i64,
}

impl NewFranklinOp {
    pub fn prepare_stored_op(
        franklin_op: &FranklinOp,
        block: BlockNumber,
        fee_account: AccountId,
    ) -> Self {
        Self {
            block_num: i64::from(block),
            operation: serde_json::to_value(franklin_op.clone()).unwrap(),
            fee_account: i64::from(fee_account),
        }
    }
}

#[derive(Debug, Clone, Queryable, QueryableByName)]
#[table_name = "executed_transactions"]
pub struct StoredExecutedTransaction {
    pub id: i32,
    pub block_number: i64,
    pub tx_hash: Vec<u8>,
    pub operation: Option<Value>,
    pub success: bool,
    pub fail_reason: Option<String>,
    pub block_index: Option<i32>,
}

#[derive(Debug, Clone, Insertable)]
#[table_name = "executed_transactions"]
pub struct NewExecutedTransaction {
    pub block_number: i64,
    pub tx_hash: Vec<u8>,
    pub operation: Option<Value>,
    pub success: bool,
    pub fail_reason: Option<String>,
    pub block_index: Option<i32>,
}
