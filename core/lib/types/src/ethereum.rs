//! Common primitives for the Ethereum network interaction.
// Built-in deps
// External uses
use thiserror::Error;
// Local uses
use crate::aggregated_operations::{AggregatedActionType, AggregatedOperation};
use zksync_basic_types::{H256, U256};

/// Numerical identifier of the Ethereum operation.
pub type EthOpId = i64;

/// Stored Ethereum operation.
#[derive(Debug, Clone)]
pub struct ETHOperation {
    // Numeric ID of the operation.
    pub id: i64,
    /// Type of the operation.
    pub op_type: AggregatedActionType,
    /// Optional ZKSync operation associated with Ethereum operation.
    pub op: Option<(i64, AggregatedOperation)>,
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

#[derive(Debug, Error, PartialEq)]
#[error("Unknown type of operation: {0}")]
pub struct UnknownOperationType(pub String);
