//! The declaration of the most primitive types used in zkSync network.
//! Most of them are just re-exported from the `web3` crate.

use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};

pub use web3::types::{Address, Log, TransactionReceipt, H160, H256, U128, U256};

/// Unique identifier of the token in the zkSync network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct TokenId(pub u16);

impl Deref for TokenId {
    type Target = u16;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TokenId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Unique identifier of the account in the zkSync network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct AccountId(pub u32);

impl Deref for AccountId {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AccountId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// zkSync network block sequential index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct BlockNumber(pub u32);

impl Deref for BlockNumber {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BlockNumber {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// zkSync account nonce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct Nonce(pub u32);

impl Deref for Nonce {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Nonce {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Unique identifier of the priority operation in the zkSync network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct PriorityOpId(pub u64);

impl Deref for PriorityOpId {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PriorityOpId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Block number in the Ethereum network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct EthBlockId(pub u64);

impl Deref for EthBlockId {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for EthBlockId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
