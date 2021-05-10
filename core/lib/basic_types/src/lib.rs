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

basic_type!(
    /// Unique identifier of the token in the zkSync network.
    TokenId,
    u32
);

basic_type!(
    /// Unique identifier of the account in the zkSync network.
    AccountId,
    u32
);

basic_type!(
    /// zkSync network block sequential index.
    BlockNumber,
    u32
);

basic_type!(
    /// zkSync account nonce.
    Nonce,
    u32
);

basic_type!(
    /// Unique identifier of the priority operation in the zkSync network.
    PriorityOpId,
    u64
);

basic_type!(
    /// Block number in the Ethereum network.
    EthBlockId,
    u64
);

basic_type!(
    /// Unique identifier of the zkSync event.
    EventId,
    u64
);
