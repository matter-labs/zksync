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

// TODO: add more info (ZKS-108).
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

/// Raw response of the [`get_account_transactions_receipts`] query.
///
/// [`get_account_transactions_receipts`]: super::OperationsExtSchema::get_account_transactions_receipts()
#[derive(Debug, FromRow, PartialEq)]
pub struct AccountTxReceiptResponse {
    /// The block containing the transaction.
    pub block_number: i64,
    /// Transaction index in block.
    ///
    /// Absent for rejected transactions.
    pub block_index: Option<i32>,
    /// Raw transaction hash bytes.
    pub tx_hash: Vec<u8>,
    /// Success status.
    pub success: bool,
    /// Reason why transaction has been rejected.
    ///
    /// May only exists for unsuccessful transactions.
    pub fail_reason: Option<String>,
    /// The raw hash bytes of the corresponding "COMMIT" Ethereum operation for block with
    /// given transaction.
    ///
    /// May only exists for successful transactions.
    pub commit_tx_hash: Option<Vec<u8>>,
    /// The raw hash bytes of the corresponding "VERIFY" Ethereum operation for block with
    /// given transaction.
    ///
    /// May only exists for successful transactions.
    pub verify_tx_hash: Option<Vec<u8>>,
}

/// Raw response of the [`get_account_operations_receipts`] query.
///
/// [`get_account_operations_receipts`]: super::OperationsExtSchema::get_account_operations_receipts()
#[derive(Debug, FromRow, PartialEq)]
pub struct AccountOpReceiptResponse {
    /// The block containing the operation.
    pub block_number: i64,
    /// Operation index in block.
    pub block_index: i32,
    /// Raw operation hash bytes.
    pub eth_hash: Vec<u8>,
    /// The raw hash bytes of the corresponding "COMMIT" Ethereum operation for block with
    /// given priority operation.
    pub commit_tx_hash: Option<Vec<u8>>,
    /// The raw hash bytes of the corresponding "VERIFY" Ethereum operation for block with
    /// given priority operation.
    pub verify_tx_hash: Option<Vec<u8>>,
}
