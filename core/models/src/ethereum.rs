//! Common primitives for the Ethereum network interaction.
// Built-in deps
use std::str::FromStr;
// External uses
use web3::types::{H256, U256};

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

impl OperationType {
    pub fn to_string(&self) -> String {
        match self {
            Self::Commit => "commit".into(),
            Self::Verify => "verify".into(),
            Self::Withdraw => "withdraw".into(),
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
#[derive(Debug, Clone, PartialEq)]
pub struct ETHOperation {
    // Numeric ID of the operation.
    pub id: i64,
    /// Type of the operation.
    pub op_type: OperationType,
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
