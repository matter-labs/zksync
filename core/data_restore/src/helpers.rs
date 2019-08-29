use bigdecimal::BigDecimal;
use bitvec::prelude::*;
use ethabi::Contract;
use franklin_crypto::circuit::float_point::parse_float_to_u128;
use models::abi::{PROD_PLASMA, TEST_PLASMA_ALWAYS_VERIFY};
use models::config::RuntimeConfig;
use models::plasma::params as plasma_constants;
use tiny_keccak::keccak256;
use web3::types::{Address, H256};

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
        let config = RuntimeConfig::new();
        Self {
            web3_endpoint: config.data_restore_http_endpoint_string, //"https://rinkeby.infura.io/".to_string(),
            franklin_contract: ethabi::Contract::load(PROD_PLASMA.0)
                .expect("Cant get plasma contract in data restore config"),
            franklin_contract_address: config
                .data_restore_franklin_contract_address
                .as_str()
                .parse()
                .expect("Cant create data restore config"), //"4fbf331db438c88a83b1316d072b7d73d8366367".parse().unwrap()
        }
    }
}

/// Infura web3 endpoints
#[derive(Debug, Copy, Clone)]
pub enum InfuraEndpoint {
    /// Mainnet Infura endpoint
    Mainnet,
    /// Rinkeby Infura endpoint
    Rinkeby,
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

/// Returns BigDecimal repr of amount bytes slice
///
/// # Arguments
///
/// * `bytes` - amount bytes slice
///
pub fn amount_bytes_slice_to_big_decimal(bytes: &[u8]) -> BigDecimal {
    let vec = bytes.to_vec();
    let bit_vec: BitVec = vec.into();
    let mut bool_vec: Vec<bool> = vec![];
    for i in bit_vec {
        bool_vec.push(i);
    }
    let amount_u128: u128 = parse_float_to_u128(
        bool_vec,
        plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap_or(0);
    let amount_u64 = amount_u128 as u64;
    // amount_f64 = amount_f64 / f64::from(1000000);
    BigDecimal::from(amount_u64)
}

/// Returns BigDecimal repr of fee bytes slice
///
/// # Arguments
///
/// * `bytes` - fee bytes slice
///
pub fn fee_bytes_slice_to_big_decimal(byte: u8) -> BigDecimal {
    let bit_vec: BitVec = BitVec::from_element(byte);
    let mut bool_vec: Vec<bool> = vec![];
    for i in bit_vec {
        bool_vec.push(i);
    }
    let fee_u128: u128 = parse_float_to_u128(
        bool_vec,
        plasma_constants::FEE_EXPONENT_BIT_WIDTH,
        plasma_constants::FEE_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap_or(0);
    let fee_u64 = fee_u128 as u64;
    // fee_f64 = fee_f64 / f64::from(1000000);
    BigDecimal::from(fee_u64)
}

/// Specific errors that may occure during data restoring
#[derive(Debug, Clone)]
pub enum DataRestoreError {
    /// Unknown error with description
    Unknown(String),
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
