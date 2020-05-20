// External imports
use chrono::NaiveDateTime;
use diesel::sql_types::{BigInt, Bool, Jsonb, Nullable, Text, Timestamp};
use serde_derive::{Deserialize, Serialize};
use serde_json::value::Value;
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

#[derive(Debug, Serialize, Deserialize, QueryableByName, PartialEq)]
pub struct TransactionsHistoryItem {
    #[sql_type = "Nullable<Text>"]
    pub hash: Option<String>,

    #[sql_type = "Nullable<BigInt>"]
    pub pq_id: Option<i64>,

    #[sql_type = "Jsonb"]
    pub tx: Value,

    #[sql_type = "Nullable<Bool>"]
    pub success: Option<bool>,

    #[sql_type = "Nullable<Text>"]
    pub fail_reason: Option<String>,

    #[sql_type = "Bool"]
    pub commited: bool,

    #[sql_type = "Bool"]
    pub verified: bool,

    #[sql_type = "Timestamp"]
    pub created_at: NaiveDateTime,
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
    pub tx_type: String,     // all
    pub from: String,        // transfer(from) | deposit(our contract) | withdraw(sender)
    pub to: String,          // transfer(to) | deposit(sender) | withdraw(our contract)
    pub token: i32,          // all
    pub amount: String,      // all
    pub fee: Option<String>, // means Sync fee, not eth. transfer(sync fee), deposit(none), withdraw(Sync fee)
    pub block_number: i64,   // all
    pub nonce: i64,          // all txs
    pub created_at: String,
    pub fail_reason: Option<String>,
    pub tx: Value,
}
