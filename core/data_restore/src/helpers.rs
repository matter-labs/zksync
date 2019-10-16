use ethabi::Contract;
use models::abi::FRANKLIN_CONTRACT;
use serde_json;
use std::str::FromStr;
use tiny_keccak::keccak256;
use web3::futures::Future;
use web3::types::{Address, H256};
use web3::types::{Transaction, TransactionId};

use lazy_static::lazy_static;
use std::env;

pub const FUNC_NAME_HASH_LENGTH: usize = 4;

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
    /// Franklin contract genesis block number: u64
    pub genesis_block_number: u64,
    /// Franklin contract creation tx hash
    pub genesis_tx_hash: H256,
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
            franklin_contract_address: env::var("CONTRACT_ADDR")
                .expect("CONTRACT_ADDR env missing")
                .as_str()
                .parse()
                .expect("Cant create data restore config"), //"4fbf331db438c88a83b1316d072b7d73d8366367".parse().unwrap()
            genesis_block_number: u64::from_str_radix(
                std::env::var("FRANKLIN_GENESIS_NUMBER")
                    .expect("FRANKLIN_GENESIS_NUMBER env missing")
                    .as_str(),
                10,
            )
            .expect("Cant get genesis number"), // 0
            genesis_tx_hash: H256::from_str(
                std::env::var("GENESIS_TX_HASH")
                    .expect("GENESIS_TX_HASH env missing")
                    .as_str(),
            )
            .expect("Cant get genesis tx hash"),
        }
    }
}

impl Default for DataRestoreConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Return Ethereum transaction input data
///
/// # Arguments
///
/// * `transaction` - Ethereum transaction description
///
pub fn get_input_data_from_ethereum_transaction(
    transaction: &Transaction,
) -> Result<Vec<u8>, DataRestoreError> {
    let input_data = transaction.clone().input.0;
    if input_data.len() > FUNC_NAME_HASH_LENGTH {
        return Ok(input_data[FUNC_NAME_HASH_LENGTH..input_data.len()].to_vec());
    } else {
        return Err(DataRestoreError::NoData(
            "No commitment data in tx".to_string(),
        ));
    }
}

/// Return Ethereum transaction description
///
/// # Arguments
///
/// * `transaction_hash` - The identifier of the particular Ethereum transaction
///
pub fn get_ethereum_transaction(transaction_hash: &H256) -> Result<Transaction, DataRestoreError> {
    let tx_id = TransactionId::Hash(transaction_hash.clone());
    let (_eloop, transport) =
        web3::transports::Http::new(DATA_RESTORE_CONFIG.web3_endpoint.as_str())
            .map_err(|_| DataRestoreError::WrongEndpoint)?;
    let web3 = web3::Web3::new(transport);
    let web3_transaction = web3
        .eth()
        .transaction(tx_id)
        .wait()
        .map_err(|e| DataRestoreError::Unknown(e.to_string()))?
        .ok_or(DataRestoreError::NoData("No tx by this hash".to_string()))?;
    Ok(web3_transaction)
}

/// Specific errors that may occure during data restoring
#[derive(Debug, Clone)]
pub enum DataRestoreError {
    /// Unknown error with description
    Unknown(String),
    /// Storage error with description
    Storage(String),
    /// Wrong data with description
    WrongData(String),
    /// Got no data with description
    NoData(String),
    /// Wrong endpoint
    WrongEndpoint,
    /// Updating failed with description
    StateUpdate(String),
}

impl std::string::ToString for DataRestoreError {
    fn to_string(&self) -> String {
        match self {
            DataRestoreError::Unknown(text) => format!("Unknown {}", text),
            DataRestoreError::Storage(text) => format!("Storage {}", text),
            DataRestoreError::WrongData(text) => format!("Wrong data {}", text),
            DataRestoreError::NoData(text) => format!("No data {}", text),
            DataRestoreError::WrongEndpoint => "Wrong endpoint".to_owned(),
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
