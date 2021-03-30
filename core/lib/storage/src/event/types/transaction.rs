// Built-in uses

// External uses
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
// Workspace uses
// Local uses

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    Queued,
    Committed,
    Finalized,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionEvent {
    pub tx_hash: Vec<u8>,
    pub account_id: i64,
    pub token_id: i32,
    pub block_number: i64,
    pub status: TransactionStatus,
    pub fail_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}
