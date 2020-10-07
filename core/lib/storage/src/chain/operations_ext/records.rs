// External imports
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::value::Value;
use sqlx::FromRow;
// Workspace imports
// Local imports
use crate::prover::records::ProverRun;

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountTransaction {
    pub tx: Value,
    pub tx_hash: String,
    pub success: bool,
    pub fail_reason: Option<String>,
    pub committed: bool,
    pub verified: bool,
}

#[derive(Debug, Serialize, Deserialize, FromRow, PartialEq)]
pub struct AccountCreatedAt {
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, FromRow, PartialEq)]
pub struct TransactionsHistoryItem {
    pub tx_id: String,
    pub hash: Option<String>,
    pub eth_block: Option<i64>,
    pub pq_id: Option<i64>,
    pub tx: Value,
    pub success: Option<bool>,
    pub fail_reason: Option<String>,
    pub commited: bool,
    pub verified: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxReceiptResponse {
    pub tx_hash: String,
    pub block_number: i64,
    pub success: bool,
    pub verified: bool,
    pub fail_reason: Option<String>,
    pub prover_run: Option<ProverRun>,
}

// TODO: jazzandrock add more info(?)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PriorityOpReceiptResponse {
    pub committed: bool,
    pub verified: bool,
    pub prover_run: Option<ProverRun>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TxByHashResponse {
    pub tx_type: String, // all
    pub from: String,    // transfer(from) | deposit(our contract) | withdraw(sender)
    pub to: String,      // transfer(to) | deposit(sender) | withdraw(our contract)
    pub token: i32,
    pub amount: String,      // all
    pub fee: Option<String>, // means Sync fee, not eth. transfer(sync fee), deposit(none), withdraw(Sync fee)
    pub block_number: i64,   // all
    pub nonce: i64,          // all txs
    pub created_at: String,
    pub fail_reason: Option<String>,
    pub tx: Value,
}
