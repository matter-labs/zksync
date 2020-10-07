//! This module contains different config structures for different types of
//! load tests.
//!
//! Currently, there are two main types of configs:
//! - loadtest config, intended for tests measuring the performance of the
//!   server under the pressure;
//! - real-life config, intended for tests ensuring the durability of the
//!   server in real-life conditions.

// Built-in imports
// External uses
use serde::Deserialize;
use web3::types::H256;
// Workspace uses
use zksync_types::Address;

/// Information about Ethereum account.
#[derive(Debug, Clone, Deserialize)]
pub struct AccountInfo {
    pub address: Address,
    pub private_key: H256,
}
