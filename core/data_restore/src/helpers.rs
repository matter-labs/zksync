use bigdecimal::BigDecimal;
use bitvec::prelude::*;
use ethabi::Contract;
use serde_json;
use std::str::FromStr;
use franklin_crypto::circuit::float_point::parse_float_to_u128;
use models::abi::FRANKLIN_CONTRACT;
use tiny_keccak::keccak256;
use web3::types::{Address, H256};

use lazy_static::lazy_static;
use std::env;

lazy_static! {
    pub static ref DATA_RESTORE_CONFIG: DataRestoreConfig = DataRestoreConfig::new();
}

/// Configuratoin of DataRestore driver
#[derive(Debug, Clone)]
pub struct DataRestoreConfig {
    /// Web3 endpoint url string
    pub web3_endpoint: String,
    /// Provides Ethereum Franklin contract unterface
    pub franklin_contract: Contract,
    /// Ethereum Franklin contract address is type of H160
    pub franklin_contract_address: Address,
}

impl DataRestoreConfig {
    /// Return the configuration for setted Infura web3 endpoint
    pub fn new() -> Self {
        let abi_string = serde_json::Value::from_str(FRANKLIN_CONTRACT)
            .expect("Cant get plasma contract")
            .get("abi")
            .expect("Cant get plasma contract abi")
            .to_string();
        Self {
            web3_endpoint: env::var("WEB3_URL").expect("WEB3_URL env missing"), //"https://rinkeby.infura.io/".to_string(),
            franklin_contract: ethabi::Contract::load(abi_string.as_bytes())
                .expect("Cant get plasma contract in data restore config"),
            franklin_contract_address: env::var("CONTRACT_ADDR").expect("CONTRACT_ADDR env missing")
                .as_str()
                .parse()
                .expect("Cant create data restore config"), //"4fbf331db438c88a83b1316d072b7d73d8366367".parse().unwrap()
        }
    }
}

impl Default for DataRestoreConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns bytes vec of keccak256 hash from bytes
///
/// # Arguments
///
/// * `bytes` - ref to bytes array
///
pub fn keccak256_hash(bytes: &[u8]) -> Vec<u8> {
    keccak256(bytes).iter().cloned().collect()
}

/// Returns keccak256 topic hash (H256) from topic str
///
/// # Arguments
///
/// * `topic` - indexed func name and args, represented in ref to str
///
pub fn get_topic_keccak_hash(topic: &str) -> web3::types::H256 {
    let topic_data: Vec<u8> = From::from(topic);
    let topic_data_vec: &[u8] = topic_data.as_slice();
    let topic_keccak_data: Vec<u8> = keccak256_hash(topic_data_vec);
    let topic_keccak_data_vec: &[u8] = topic_keccak_data.as_slice();
    H256::from_slice(topic_keccak_data_vec)
}

/// Specific errors that may occure during data restoring
#[derive(Debug, Clone)]
pub enum DataRestoreError {
    /// Unknown error with description
    Unknown(String),
    /// Storage error with description
    Storage(String),
    /// Wrong type
    WrongType,
    /// Got no data with description
    NoData(String),
    /// Account doesn't exists
    NonexistentAccount,
    /// Wrong amount
    WrongAmount,
    /// Wrong endpoint
    WrongEndpoint,
    /// Wrong public key
    WrongPubKey,
    /// Double exit in chain
    DoubleExit,
    /// Updating failed with description
    StateUpdate(String),
}

impl std::string::ToString for DataRestoreError {
    fn to_string(&self) -> String {
        match self {
            DataRestoreError::Unknown(text) => format!("Unknown {}", text),
            DataRestoreError::Storage(text) => format!("Storage {}", text),
            DataRestoreError::WrongType => "Wrong type".to_owned(),
            DataRestoreError::NoData(text) => format!("No data {}", text),
            DataRestoreError::NonexistentAccount => "Nonexistent account".to_owned(),
            DataRestoreError::WrongAmount => "Wrong amount".to_owned(),
            DataRestoreError::WrongEndpoint => "Wrong endpoint".to_owned(),
            DataRestoreError::WrongPubKey => "Wrong pubkey".to_owned(),
            DataRestoreError::DoubleExit => "Double exit".to_owned(),
            DataRestoreError::StateUpdate(text) => format!("Error during state update {}", text),
        }
    }
}

impl std::convert::From<&str> for DataRestoreError {
    fn from(a: &str) -> Self {
        DataRestoreError::Unknown(a.to_string())
    }
}

impl std::convert::From<String> for DataRestoreError {
    fn from(a: String) -> Self {
        DataRestoreError::Unknown(a)
    }
}
