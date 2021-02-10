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
    /// Number of the last block for which was committed.
    pub last_committed_block: usize,
    /// Number of the last block for which was verified.
    pub last_verified_block: usize,
    /// Number of the last block for which was executed.
    pub last_executed_block: usize,
}

impl From<StorageETHStats> for ETHStats {
    fn from(stored: StorageETHStats) -> Self {
        Self {
            last_committed_block: stored.last_committed_block as usize,
            last_verified_block: stored.last_verified_block as usize,
            last_executed_block: stored.last_executed_block as usize,
        }
    }
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
