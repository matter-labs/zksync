//! The declaration of the most primitive types used in zkSync network.
//! Most of them are just re-exported from the `web3` crate.

pub use web3::types::{Address, Log, TransactionReceipt, H160, H256, U128, U256};

pub type TokenId = u16;
pub type AccountId = u32;
pub type BlockNumber = u32;
pub type Nonce = u32;
