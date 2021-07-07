//! Unlike the rest `records` modules in the `storage` crate, `operations_ext::records`
//! rather consists of structures that represent database query results. This is needed
//! for employing `sqlx::query_as` macro for compile-time type checks.

// External imports
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::value::Value;
use sqlx::FromRow;
// Workspace imports
// Local imports
use crate::prover::records::ProverRun;

/// Wrapper for date and time of the first executed transaction
/// for the account.
#[derive(Debug, Serialize, Deserialize, FromRow, PartialEq)]
pub struct AccountCreatedAt {
    pub created_at: DateTime<Utc>,
}

/// A single entry from the raw response of the [`get_account_transactions_history`] query.
///
/// [`get_account_transactions_history`]: super::OperationsExtSchema::get_account_transactions_history()
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

/// Stored information resulted from executing the transaction.
/// Obtained from the operations schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxReceiptResponse {
    pub tx_hash: String,
    pub block_number: i64,
    pub success: bool,
    pub verified: bool,
    pub fail_reason: Option<String>,
    pub prover_run: Option<ProverRun>,
}

/// Stored information resulted from executing the priority operation.
/// Obtained from the operations schema.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PriorityOpReceiptResponse {
    pub committed: bool,
    pub verified: bool,
    pub prover_run: Option<ProverRun>,
}

/// Stored executed operation (can be both L1 or L2)
/// unified under a single interface for the explorer.
#[derive(Debug, Serialize, Deserialize)]
pub struct TxByHashResponse {
    pub tx_type: String,
    /// Address of transaction sender for `Transfer`, `Withdraw` and `ChangePubKey`.
    ///
    /// Target's address in case of `ForcedExit`.
    ///
    /// Author's address in L1 for `Deposit` and `FullExit`.
    pub from: String,
    /// Receiver's address for `Transfer`.
    ///
    /// Author's address in L1 for `Withdraw` and 'FullExit'.
    ///
    /// New public key hash for `ChangePubKey`.
    ///
    /// Sender's address for `Deposit`.
    ///
    /// Target's address in case of `ForcedExit`.
    pub to: String,
    pub token: i32,
    pub amount: String,
    /// Fee paid in the zkSync network.
    /// `None` for priority operations.
    ///
    /// Can also be `None` for very old `ChangePubKey` operations.
    pub fee: Option<String>,
    pub block_number: i64,
    pub nonce: i64,
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

#[derive(Debug, FromRow, PartialEq)]
pub struct InBlockBatchTx {
    pub tx_hash: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub success: bool,
    pub block_number: i64,
}

#[derive(Debug, FromRow, PartialEq)]
pub struct StorageTxReceipt {
    pub tx_hash: Vec<u8>,
    pub block_number: Option<i64>,
    pub success: Option<bool>,
    pub fail_reason: Option<String>,
    pub eth_block: Option<i64>,
    pub priority_op_serialid: Option<i64>,
}

pub struct StorageTxData {
    pub tx_hash: Vec<u8>,
    pub block_number: Option<i64>,
    pub op: Value,
    pub success: Option<bool>,
    pub fail_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub eth_hash: Option<Vec<u8>>,
    pub priority_op_serialid: Option<i64>,
    pub eth_sign_data: Option<serde_json::Value>,
}
