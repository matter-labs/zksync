//! Common primitives for the Ethereum network interaction.
// Built-in deps
use std::{convert::TryFrom, fmt, str::FromStr};
// External uses
use ethabi::{decode, ParamType};
use serde::{Deserialize, Serialize};
// Local uses
use crate::{Action, Operation};
use zksync_basic_types::{Log, H256, U256};

/// Numerical identifier of the Ethereum operation.
pub type EthOpId = i64;

/// Type of the transactions sent to the Ethereum network.
#[derive(Debug, Clone, Copy, PartialEq)]
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
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let op = match s {
            "commit" => Self::Commit,
            "verify" => Self::Verify,
            "withdraw" => Self::Withdraw,
            _ => anyhow::bail!("Unknown type of operation: {}", s),
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

    /// Checks whether this object relates to the `Verify` zkSync operation.
    pub fn is_verify(&self) -> bool {
        if let Some(op) = &self.op {
            matches!(op.action, Action::Verify { .. })
        } else {
            false
        }
    }

    /// Completes the object state with the data obtained from the database.
    pub fn complete(&mut self, inserted_data: InsertedOperationResponse) {
        self.id = inserted_data.id;
        self.nonce = inserted_data.nonce;
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

/// Structure representing the result of the insertion of the Ethereum
/// operation into the database.
/// Contains the assigned nonce and ID for the operation.
pub struct InsertedOperationResponse {
    /// Unique numeric identifier of the Ethereum operation.
    pub id: i64,
    /// Nonce assigned for the Ethereum operation. Meant to be used for all the
    /// transactions sent within one particular Ethereum operation.
    pub nonce: U256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteWithdrawalsTx {
    pub tx_hash: H256,
    pub pending_withdrawals_queue_start_index: u32,
    pub pending_withdrawals_queue_end_index: u32,
}

impl TryFrom<Log> for CompleteWithdrawalsTx {
    type Error = anyhow::Error;

    fn try_from(event: Log) -> Result<CompleteWithdrawalsTx, anyhow::Error> {
        let mut decoded_event = decode(
            &[
                ParamType::Uint(32), // queueStartIndex
                ParamType::Uint(32), // queueEndIndex
            ],
            &event.data.0,
        )
        .map_err(|e| anyhow::format_err!("Event data decode: {:?}", e))?;

        Ok(CompleteWithdrawalsTx {
            tx_hash: event
                .transaction_hash
                .expect("complete withdrawals transaction should have hash"),
            pending_withdrawals_queue_start_index: decoded_event
                .remove(0)
                .to_uint()
                .as_ref()
                .map(U256::as_u32)
                .expect("pending_withdrawals_queue_start_index value conversion failed"),
            pending_withdrawals_queue_end_index: decoded_event
                .remove(0)
                .to_uint()
                .as_ref()
                .map(U256::as_u32)
                .expect("pending_withdrawals_queue_end_index value conversion failed"),
        })
    }
}
