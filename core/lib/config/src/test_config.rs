// Built-in deps
use std::fs;
// External uses
use serde::Deserialize;
// Workspace uses
use zksync_basic_types::{Address, H256};
// Local uses

/// Transforms relative path like `constant/eip1271.json` into full path like
/// `$ZKSYNC_HOME/etc/test_config/constant/eip1271.json`.
fn config_path(postfix: &str) -> String {
    let home = std::env::var("ZKSYNC_HOME").expect("ZKSYNC_HOME variable must be set");

    format!("{}/etc/test_config/{}", home, postfix)
}

fn load_json(path: &str) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).expect("Invalid config path"))
        .expect("Invalid config format")
}

/// Takes name of the config, extends it to the constant and volatile config paths,
/// loads them and merged into on object.
fn merge_configs(config: &str) -> serde_json::Value {
    let mut constant_config = load_json(&config_path(&format!("constant/{}", config)));
    let mut volatile_config = load_json(&config_path(&format!("volatile/{}", config)));

    constant_config
        .as_object_mut()
        .expect("Cannot merge not at object")
        .append(volatile_config.as_object_mut().unwrap());

    constant_config
}

/// Configuration for EIP1271-compatible test smart wallet.
#[derive(Debug, Deserialize)]
pub struct EIP1271Config {
    /// Private key of the account owner (to sign transactions).
    pub owner_private_key: H256,
    /// Address of the account owner (set in contract).
    pub owner_address: Address,
    /// Address of the smart wallet contract.
    pub contract_address: Address,
}

impl EIP1271Config {
    pub fn load() -> Self {
        let object = merge_configs("eip1271.json");
        serde_json::from_value(object).expect("Cannot deserialize EIP1271 test config")
    }
}

/// Common Ethereum parameters.
#[derive(Debug, Deserialize)]
pub struct EthConfig {
    /// Address of the local Ethereum node.
    pub web3_url: String,
}

impl EthConfig {
    pub fn load() -> Self {
        let object = load_json(&config_path("constant/eth.json"));
        serde_json::from_value(object).expect("Cannot deserialize Ethereum test config")
    }
}

#[derive(Debug)]
pub struct TestConfig {
    pub eip1271: EIP1271Config,
    pub eth: EthConfig,
}

impl TestConfig {
    pub fn load() -> Self {
        Self {
            eip1271: EIP1271Config::load(),
            eth: EthConfig::load(),
        }
    }
}
