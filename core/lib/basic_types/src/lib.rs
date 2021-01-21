//! The declaration of the most primitive types used in zkSync network.
//! Most of them are just re-exported from the `web3` crate.

#[macro_use]
mod macros;

use serde::{Deserialize, Serialize};
use std::fmt;
use std::num::ParseIntError;
use std::ops::{Add, Deref, DerefMut, Sub};
use std::str::FromStr;

pub use web3::types::{Address, Log, TransactionReceipt, H160, H256, U128, U256};

/// Unique identifier of the token in the zkSync network.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, PartialOrd, Ord, Default,
)]
pub struct TokenId(pub u16);
impl_deref_and_deref_mut!(TokenId, u16);

/// Unique identifier of the account in the zkSync network.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, PartialOrd, Ord, Default,
)]
pub struct AccountId(pub u32);
impl_deref_and_deref_mut!(AccountId, u32);
impl_from_str_and_display!(AccountId, u32);

/// zkSync network block sequential index.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, PartialOrd, Ord, Default,
)]
pub struct BlockNumber(pub u32);
impl_deref_and_deref_mut!(BlockNumber, u32);
impl_add_and_sub_with_base_type!(BlockNumber, u32);

/// zkSync account nonce.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, PartialOrd, Ord, Default,
)]
pub struct Nonce(pub u32);
impl_deref_and_deref_mut!(Nonce, u32);
impl_add_and_sub_with_base_type!(Nonce, u32);

/// Unique identifier of the priority operation in the zkSync network.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, PartialOrd, Ord, Default,
)]
pub struct PriorityOpId(pub u64);
impl_deref_and_deref_mut!(PriorityOpId, u64);
impl_from_str_and_display!(PriorityOpId, u64);

/// Block number in the Ethereum network.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, PartialOrd, Ord, Default,
)]
pub struct EthBlockId(pub u64);
impl_deref_and_deref_mut!(EthBlockId, u64);
