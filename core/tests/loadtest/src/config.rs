//! This module contains different config structures for different types of
//! load tests.
//!
//! Currently, there are two main types of configs:
//! - loadtest config, intended for tests measuring the performance of the
//!   server under the pressure;
//! - real-life config, intended for tests ensuring the durability of the
//!   server in real-life conditions.

// Built-in imports
use std::{fs, path::Path};
// External uses
use serde::{Deserialize, Serialize};
use web3::types::H256;
// Workspace uses
use zksync::Network;
use zksync_types::{Address, TokenLike};
// Local uses
use crate::scenarios::ScenarioConfig;

/// Information about Ethereum account.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct AccountInfo {
    pub address: Address,
    pub private_key: H256,
    /// The name of the token used for testing.
    pub token_name: TokenLike,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
pub struct NetworkConfig {
    /// Network kind used for testing.
    pub name: Network,
    /// Fee for the Ethereum transactions in gwei.
    #[serde(default)]
    pub eth_fee: Option<u64>,
    /// Fee for the zkSync transactions in gwei.
    #[serde(default)]
    pub zksync_fee: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    /// Information about Ethereum account.
    pub main_wallet: AccountInfo,
    /// Network configuration.
    pub network: NetworkConfig,
    /// Loadtest scenarios.
    pub scenarios: Vec<ScenarioConfig>,
}

impl Default for Config {
    fn default() -> Self {
        let config_str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/config/localhost.toml"
        ));
        toml::from_str(config_str).unwrap()
    }
}

impl Config {
    /// Reads config from the given TOML file.
    pub fn from_toml(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        toml::from_str(&content).map_err(From::from)
    }
}
