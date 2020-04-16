// External imports
use chrono::prelude::*;
use diesel::sql_types::{BigInt, Bool, Int4, Jsonb, Nullable, Text, Timestamp};
use serde_derive::{Deserialize, Serialize};
use serde_json::value::Value;
// Workspace imports
// Local imports
use crate::prover::records::ProverRun;
use crate::schema::*;

#[derive(Debug, Insertable)]
#[table_name = "mempool"]
pub struct InsertTx {
    pub hash: Vec<u8>,
    pub primary_account_address: Vec<u8>,
    pub nonce: i64,
    pub tx: Value,
}

#[derive(Debug, Queryable)]
pub struct ReadTx {
    pub hash: Vec<u8>,
    pub primary_account_address: Vec<u8>,
    pub nonce: i64,
    pub tx: Value,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, QueryableByName)]
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
#[derive(Debug, Serialize, Deserialize)]
pub struct PriorityOpReceiptResponse {
    pub committed: bool,
    pub verified: bool,
    pub prover_run: Option<ProverRun>,
}

#[derive(Debug, Serialize, Deserialize, Queryable, QueryableByName)]
pub struct TxByHashResponse {
    #[sql_type = "Text"]
    pub tx_type: String, // all

    #[sql_type = "Text"]
    pub from: String, // transfer(from) | deposit(our contract) | withdraw(sender)

    #[sql_type = "Text"]
    pub to: String, // transfer(to) | deposit(sender) | withdraw(our contract)

    #[sql_type = "Int4"]
    pub token: i32, // all

    #[sql_type = "Text"]
    pub amount: String, // all

    #[sql_type = "Nullable<Text>"]
    pub fee: Option<String>, // means Sync fee, not eth. transfer(sync fee), deposit(none), withdraw(Sync fee)

    #[sql_type = "BigInt"]
    pub block_number: i64, // all

    #[sql_type = "BigInt"]
    pub nonce: i64, // all txs

    #[sql_type = "Timestamp"]
    pub created_at: String,

    #[sql_type = "Nullable<Text>"]
    pub fail_reason: Option<String>,
}
