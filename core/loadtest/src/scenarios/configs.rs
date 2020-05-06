//! This module contains different config structures for different types of
//! load tests.
//!
//! Currently, there are two main types of configs:
//! - loadtest config, intended for tests measuring the performance of the
//!   server under the pressure;
//! - real-life config, intended for tests ensuring the durability of the
//!   server in real-life conditions.

// Built-in imports
use std::path::PathBuf;
// External uses
use serde_derive::Deserialize;
use web3::types::H256;
// Workspace uses
use models::node::Address;

/// Information about Ethereum account.
#[derive(Debug, Clone, Deserialize)]
pub struct AccountInfo {
    pub address: Address,
    pub private_key: H256,
}

/// Configuration of the load-test, which contains the parameters
/// for creating a pressure on the server.
#[derive(Debug, Clone, Deserialize)]
pub struct LoadTestConfig {
    pub deposit_initial_gwei: u64,
    pub n_deposits: u32,
    pub deposit_from_amount_gwei: u64,
    pub deposit_to_amount_gwei: u64,
    pub n_transfers: u32,
    pub transfer_from_amount_gwei: u64,
    pub transfer_to_amount_gwei: u64,
    pub n_withdraws: u32,
    pub withdraw_from_amount_gwei: u64,
    pub withdraw_to_amount_gwei: u64,
    pub verify_timeout_sec: u64,
    pub input_accounts: Vec<AccountInfo>,
}

impl LoadTestConfig {
    /// Loads the spec from the file given its path.
    pub fn load(filepath: &PathBuf) -> Self {
        load_json(filepath)
    }
}

/// Configuration of the real-life test, which contains the parameters
/// determining the breadth (amount of accounts involved) and the depth
/// (amount of iterations) of the test
#[derive(Debug, Clone, Deserialize)]
pub struct RealLifeConfig {
    pub n_accounts: usize,
    pub transfer_size: u64,
    pub cycles_amount: u32,
    pub input_account: AccountInfo,
}

impl RealLifeConfig {
    /// Loads the spec from the file given its path.
    pub fn load(filepath: &PathBuf) -> Self {
        load_json(filepath)
    }
}

fn load_json<T: serde::de::DeserializeOwned>(filepath: &PathBuf) -> T {
    let buffer = std::fs::read_to_string(filepath).expect("Failed to read the test spec file");
    serde_json::from_str(&buffer).expect(
        "Failed to parse config file. Ensure that you provided \
             the correct path for the type of test you're about to run",
    )
}
