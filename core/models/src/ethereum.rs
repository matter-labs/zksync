//! Common primitives for the Ethereum network interaction.
// Built-in deps
use std::fmt;
use std::str::FromStr;
// External uses
/// Local uses
use crate::{Action, Operation};
use web3::types::{H256, U256};

/// Numerical identifier of the Ethereum operation.
pub type EthOpId = i64;

/// Type of the transactions sent to the Ethereum network.
#[derive(Debug, Clone, PartialEq)]
pub enum OperationType {
    /// Commit action (`commitBlock` method of the smart contract).
    Commit,
    /// Verify action (`verifyBlock` method of the smart contract).
    Verify,
    /// Withdraw action (`completeWithdrawals` method of the smart contract).
    Withdraw,
}

impl fmt::Display for OperationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Commit => write!(f, "commit"),
            Self::Verify => write!(f, "verify"),
            Self::Withdraw => write!(f, "withdraw"),
        }
    }
}

impl FromStr for OperationType {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let op = match s {
            "commit" => Self::Commit,
            "verify" => Self::Verify,
            "withdraw" => Self::Withdraw,
            _ => failure::bail!("Unknown type of operation: {}", s),
        };

        Ok(op)
    }
}

/// Stored Ethereum operation.
#[derive(Debug, Clone)]
pub struct ETHOperation {
    // Numeric ID of the operation.
    pub id: i64,
    /// Type of the operation.
    pub op_type: OperationType,
    /// Optional ZKSync operation associated with Ethereum operation.
    pub op: Option<Operation>,
    /// Used nonce (fixed for all the sent transactions).
    pub nonce: U256,
    /// Deadline block of the last sent transaction.
    pub last_deadline_block: u64,
    /// Gas price used in the last sent transaction.
    pub last_used_gas_price: U256,
    /// Hashes of all the sent transactions.
    pub used_tx_hashes: Vec<H256>,
    /// Tx payload (not signed).
    pub encoded_tx_data: Vec<u8>,
    /// Flag showing if the operation was completed and
    /// confirmed on the Ethereum blockchain.
    pub confirmed: bool,
    /// Hash of the accepted Ethereum transaction (if operation
    /// is confirmed).
    pub final_hash: Option<H256>,
}

impl ETHOperation {
    /// Checks whether the transaction is considered "stuck".
    /// "Stuck" transactions are ones that were not included into any block
    /// within a desirable amount of time, and thus require re-sending with
    /// increased gas amount.
    pub fn is_stuck(&self, current_block: u64) -> bool {
        current_block >= self.last_deadline_block
    }

    /// Checks whether this object relates to the `Verify` ZK Sync operation.
    pub fn is_verify(&self) -> bool {
        if let Some(op) = &self.op {
            matches!(op.action, Action::Verify { .. })
        } else {
            false
        }
    }
}

impl PartialEq for ETHOperation {
    fn eq(&self, other: &Self) -> bool {
        // We assume that there will be no two different `ETHOperation`s with
        // the same identifiers.
        // However, the volatile fields (e.g. `used_tx_hashes` and `confirmed`) may vary
        // for the same operation in different states, so we compare them as well.
        (self.id == other.id)
            && (self.last_deadline_block == other.last_deadline_block)
            && (self.last_used_gas_price == other.last_used_gas_price)
            && (self.used_tx_hashes == other.used_tx_hashes)
            && (self.confirmed == other.confirmed)
            && (self.final_hash == other.final_hash)
    }
}
