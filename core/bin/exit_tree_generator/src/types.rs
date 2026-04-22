use std::str::FromStr;

use bigdecimal::BigDecimal;
use ethabi::ethereum_types::U256;
use num_bigint::ToBigInt;
use web3::types::Address;

/// Account data structure loaded from CSV storage.
/// Represents a zkSync account with its basic information.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct StorageAccount {
    /// Unique account identifier
    pub id: u32,
    /// Account nonce (number of transactions)
    pub nonce: u32,
    /// Ethereum address of the account (as hex string)
    pub address: String,
    /// Public key hash of the account (as hex string)
    pub pubkey_hash: String,
}

/// Balance data structure loaded from CSV storage.
/// Represents a token balance for a specific account.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct StorageBalance {
    /// Account ID that owns this balance
    pub account_id: u32,
    /// Token/coin ID for this balance
    pub coin_id: u32,
    /// Balance amount as a decimal string
    pub balance: String,
}

/// Token data structure loaded from CSV storage.
/// Maps token IDs to their Ethereum addresses.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct StorageToken {
    /// Unique token identifier
    pub id: u32,
    /// Ethereum address of the token contract
    pub address: Address,
}

/// Merkle tree leaf structure representing a (account, token, balance) combination.
/// Used for generating Merkle proofs in exit scenarios.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct MerkleTreeLeaf {
    /// Canonical leaf index used in the contract-side packed leaf hash.
    pub claim_index: u64,
    /// Ethereum address of the account
    pub account_address: Address,
    /// Ethereum address of the token
    pub token_address: Address,
    /// Balance amount as a decimal string
    pub balance: String,
}

impl MerkleTreeLeaf {
    /// Parses the decimal balance string into the uint256 value used by the contracts.
    pub fn balance_as_u256(&self) -> U256 {
        U256::from_big_endian(
            BigDecimal::from_str(&self.balance)
                .unwrap()
                .to_bigint()
                .unwrap()
                .to_bytes_be()
                .1
                .as_slice(),
        )
    }
}
