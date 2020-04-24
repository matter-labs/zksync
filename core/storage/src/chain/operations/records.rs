// External imports
use chrono::prelude::*;
use serde_json::value::Value;
// Workspace imports
// Local imports
use crate::schema::*;
use crate::utils::StoredBigUint;

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

#[derive(Debug, Clone, Insertable)]
#[table_name = "executed_priority_operations"]
pub struct NewExecutedPriorityOperation {
    pub block_number: i64,
    pub block_index: i32,
    pub operation: Value,
    pub from_account: Vec<u8>,
    pub to_account: Vec<u8>,
    pub priority_op_serialid: i64,
    pub deadline_block: i64,
    pub eth_hash: Vec<u8>,
}

#[derive(Debug, Clone, Queryable, QueryableByName)]
#[table_name = "executed_priority_operations"]
pub struct StoredExecutedPriorityOperation {
    pub id: i32,
    pub block_number: i64,
    pub block_index: i32,
    pub operation: Value,
    pub from_account: Vec<u8>,
    pub to_account: Vec<u8>,
    pub priority_op_serialid: i64,
    pub deadline_block: i64,
    pub eth_hash: Vec<u8>,
}

#[derive(Debug, Clone, Insertable)]
#[table_name = "executed_transactions"]
pub struct NewExecutedTransaction {
    pub block_number: i64,
    pub block_index: Option<i32>,
    pub tx: Value,
    pub operation: Value,
    pub tx_hash: Vec<u8>,
    pub from_account: Vec<u8>,
    pub to_account: Option<Vec<u8>>,
    pub success: bool,
    pub fail_reason: Option<String>,
    pub primary_account_address: Vec<u8>,
    pub nonce: i64,
}

#[derive(Debug, Clone, Queryable, QueryableByName)]
#[table_name = "executed_transactions"]
pub struct StoredExecutedTransaction {
    pub id: i32,
    pub block_number: i64,
    pub block_index: Option<i32>,
    pub tx: Value,
    pub operation: Value,
    pub tx_hash: Vec<u8>,
    pub from_account: Vec<u8>,
    pub to_account: Option<Vec<u8>>,
    pub success: bool,
    pub fail_reason: Option<String>,
    pub primary_account_address: Vec<u8>,
    pub nonce: i64,
}
