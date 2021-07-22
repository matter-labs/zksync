// Built-in deps
use std::fs;
// External uses
use serde::Deserialize;
// Workspace uses
use zksync_types::{Address, H256};
// Local uses

pub mod unit_vectors;

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
    /// Set of 12 words for connecting to an Ethereum wallet.
    pub test_mnemonic: String,
}

/// Common Api addresses.
#[derive(Debug, Deserialize)]
pub struct ApiConfig {
    /// Address of the rest api.
    pub rest_api_url: String,
}

macro_rules! impl_config {
    ($name_config:ident, $file:tt) => {
        impl $name_config {
            pub fn load() -> Self {
                let object = load_json(&config_path(&format!("{}.json", $file)));
                serde_json::from_value(object)
                    .expect(&format!("Cannot deserialize config from '{}'", $file))
            }
        }
    };
}

impl_config!(ApiConfig, "constant/api");
impl_config!(EthConfig, "constant/eth");

#[derive(Debug)]
pub struct TestConfig {
    pub eip1271: EIP1271Config,
    pub eth: EthConfig,
    pub api: ApiConfig,
}

impl TestConfig {
    pub fn load() -> Self {
        Self {
            eip1271: EIP1271Config::load(),
            eth: EthConfig::load(),
            api: ApiConfig::load(),
        }
    }
}
