//! Ethereum transaction utilities.
//!
//! This module contains the helper types that represent the state of
//! ZKSync and Ethereum blockchains synchronization.

// Built-in deps
// External uses
use zksync_basic_types::TransactionReceipt;
// Workspace uses
use zksync_storage::ethereum::records::ETHStats as StorageETHStats;

/// Collected statistics of the amount of operations sent to the Ethereum.
/// This structure represents the count of **operations**, and not transactions.
/// It means that if for some operation there were N txs sent, it will be counted as
/// 1 operation anyway.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ETHStats {
    /// Amount of sent commit operations.
    pub commit_ops: usize,
    /// Amount of sent verify operations.
    pub verify_ops: usize,
    /// Amount of sent withdraw operations.
    pub withdraw_ops: usize,
}

impl From<StorageETHStats> for ETHStats {
    fn from(stored: StorageETHStats) -> Self {
        Self {
            commit_ops: stored.commit_ops as usize,
            verify_ops: stored.verify_ops as usize,
            withdraw_ops: stored.withdraw_ops as usize,
        }
    }
}

/// State of the executed Ethereum transaction.
#[derive(Debug, Clone)]
pub(super) struct ExecutedTxStatus {
    /// Amount of confirmations for a block containing the transaction.
    pub confirmations: u64,
    /// Whether transaction was executed successfully or failed.
    pub success: bool,
    /// Receipt for a transaction. Will be set to `Some` only if the transaction
    /// failed during execution.
    pub receipt: Option<TransactionReceipt>,
}

/// The result of the check for the Ethereum transaction commitment.
#[derive(Debug, PartialEq)]
pub enum TxCheckOutcome {
    /// Transaction was committed and confirmed.
    Committed,
    /// Transaction is pending yet.
    Pending,
    /// Transaction is considered stuck, a replacement should be made.
    Stuck,
    /// Transaction execution failed. Receipt is boxed to reduce the enum object size.
    Failed(Box<TransactionReceipt>),
}

/// Enumeration denoting if the operation was successfully committed, or not yet.
#[derive(Debug, PartialEq, Eq)]
pub enum OperationCommitment {
    Committed,
    Pending,
}

impl Default for OperationCommitment {
    fn default() -> Self {
        Self::Pending
    }
}
