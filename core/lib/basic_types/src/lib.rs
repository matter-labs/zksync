//! The declaration of the most primitive types used in zkSync network.
//! Most of them are just re-exported from the `web3` crate.

pub use web3::types::{Address, Log, TransactionReceipt, H160, H256, U128, U256};

/// Unique identifier of the token in the zkSync network.
pub type TokenId = u16;
/// Unique identifier of the account in the zkSync network.
pub type AccountId = u32;
/// zkSync network block sequential index.
pub type BlockNumber = u32;
/// zkSync account nonce.
pub type Nonce = u32;
/// Unique identifier of the priority operation in the zkSync network.
pub type PriorityOpId = u64;
