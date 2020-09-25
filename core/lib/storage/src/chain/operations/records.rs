// External imports
use chrono::prelude::*;
use serde_json::value::Value;
use sqlx::FromRow;
// Workspace imports
// Local imports

#[derive(Debug, Clone, FromRow)]
pub struct StoredOperation {
    pub id: i64,
    pub block_number: i64,
    pub action_type: String,
    pub created_at: DateTime<Utc>,
    pub confirmed: bool,
}

#[derive(Debug, Clone, FromRow)]
pub struct StoredExecutedPriorityOperation {
    pub block_number: i64,
    pub block_index: i32,
    pub operation: Value,
    pub from_account: Vec<u8>,
    pub to_account: Vec<u8>,
    pub priority_op_serialid: i64,
    pub deadline_block: i64,
    pub eth_hash: Vec<u8>,
    pub eth_block: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct StoredExecutedTransaction {
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
    pub created_at: DateTime<Utc>,
    pub eth_sign_data: Option<serde_json::Value>,
    pub batch_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewOperation {
    pub block_number: i64,
    pub action_type: String,
}

#[derive(Debug, Clone)]
pub struct NewExecutedPriorityOperation {
    pub block_number: i64,
    pub block_index: i32,
    pub operation: Value,
    pub from_account: Vec<u8>,
    pub to_account: Vec<u8>,
    pub priority_op_serialid: i64,
    pub deadline_block: i64,
    pub eth_hash: Vec<u8>,
    pub eth_block: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
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
    pub created_at: DateTime<Utc>,
    pub eth_sign_data: Option<serde_json::Value>,
    pub batch_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct StoredPendingWithdrawal {
    pub id: i64,
    pub withdrawal_hash: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct StoredCompleteWithdrawalsTransaction {
    pub tx_hash: Vec<u8>,
    pub pending_withdrawals_queue_start_index: i64,
    pub pending_withdrawals_queue_end_index: i64,
}
