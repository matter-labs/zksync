//! Ethereum transaction utilities.
//!
//! This module contains the helper types that represent the state of
//! ZKSync and Ethereum blockchains synchronization.

// Built-in deps
use std::str::FromStr;
// External uses
use web3::types::{TransactionReceipt, H256, U256};
// Workspace uses
use eth_client::SignedCallResult;
use models::Operation;
use storage::StorageETHOperation;

/// An intermediate state of the operation to be stored on
/// the Ethereum chain.
#[derive(Debug, Clone)]
pub(super) struct OperationETHState {
    /// ZKSync operation to be stored.
    pub operation: Operation,
    /// List of sent Ethereum transactions that persist the
    /// ZKSync operation.
    /// It is empty at the beginning, and if everything goes
    /// smoothly, it will not be extended more than once.
    /// However, transactions can "stuck" and not be included in
    /// the block, so `ETHSender` may try to send more transactions
    /// to resolve the situation.
    pub txs: Vec<TransactionETHState>,
}

/// Representation of the transaction sent to the Ethereum chain.
#[derive(Debug, Clone)]
pub struct TransactionETHState {
    /// ZKSync operation identifier.
    pub op_id: i64,
    /// Block until which transaction should be committed.
    /// Exceeding this limit will make the transaction considered to be stuck.
    pub deadline_block: u64,
    /// Raw Ethereum transaction with additional meta-information.
    pub signed_tx: SignedCallResult,
}

impl From<StorageETHOperation> for TransactionETHState {
    fn from(stored: StorageETHOperation) -> Self {
        TransactionETHState {
            op_id: stored.op_id,
            deadline_block: stored.deadline_block as u64,
            signed_tx: SignedCallResult {
                raw_tx: stored.raw_tx,
                gas_price: U256::from_str(&stored.gas_price.to_string()).unwrap(),
                nonce: U256::from(stored.nonce as u128),
                hash: H256::from_slice(&stored.tx_hash),
            },
        }
    }
}

impl TransactionETHState {
    /// Checks whether the transaction is considered "stuck".
    /// "Stuck" transactions are ones that were not included into any block
    /// within a desirable amount of time, and thus require re-sending with
    /// increased gas amount.
    pub fn is_stuck(&self, current_block: u64) -> bool {
        current_block >= self.deadline_block
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
#[derive(Debug)]
pub enum TxCheckOutcome {
    /// Transaction was committed and confirmed.
    Committed,
    /// Transaction is pending yet.
    Pending,
    /// Transaction is considered stuck, a replacement should be made.
    Stuck,
    /// Transaction execution failed.
    Failed(TransactionReceipt),
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
