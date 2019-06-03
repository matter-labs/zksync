use web3::types::{H256,Address};
use tiny_keccak::keccak256;
use bigdecimal::BigDecimal;
use bitvec::prelude::*;
use sapling_crypto::circuit::float_point::parse_float_to_u128;
use std::env;
use ethabi::Contract;
use super::commons::{PROD_PLASMA, TEST_PLASMA_ALWAYS_VERIFY};
use super::models::config::RuntimeConfig;

#[derive(Debug, Clone)]
pub struct DataRestoreConfig {
    pub http_endpoint_string: String,
    pub franklin_contract: Contract,
    pub franklin_contract_address: Address,
}

impl DataRestoreConfig {
    pub fn new(network: InfuraEndpoint) -> Self {
        let config = RuntimeConfig::new();
        match network {
            InfuraEndpoint::Mainnet => {
                Self {
                    http_endpoint_string:      config.mainnet_http_endpoint_string,
                    franklin_contract:         ethabi::Contract::load(PROD_PLASMA.0).unwrap(),
                    franklin_contract_address: config.mainnet_franklin_contract_address.as_str().parse().unwrap(),
                }
            },
            InfuraEndpoint::Rinkeby => {
                Self {
                    http_endpoint_string:      config.rinkeby_http_endpoint_string,
                    franklin_contract:         ethabi::Contract::load(TEST_PLASMA_ALWAYS_VERIFY.0).unwrap(),
                    franklin_contract_address: config.rinkeby_franklin_contract_address.as_str().parse().unwrap(),
                }
            },
        }
    }
}

/// Amount bit widths
pub const AMOUNT_EXPONENT_BIT_WIDTH: usize = 5;
pub const AMOUNT_MANTISSA_BIT_WIDTH: usize = 11;

/// Fee bit widths
pub const FEE_EXPONENT_BIT_WIDTH: usize = 5;
pub const FEE_MANTISSA_BIT_WIDTH: usize = 3;

#[derive(Debug, Copy, Clone)]
pub enum InfuraEndpoint {
    Mainnet,
    Rinkeby
}

pub fn keccak256_hash(bytes: &[u8]) -> Vec<u8> {
    keccak256(bytes).into_iter().cloned().collect()
}

pub fn get_topic_keccak_hash(topic: &str) -> web3::types::H256 {
    let topic_data: Vec<u8> = From::from(topic);
    let topic_data_vec: &[u8] = topic_data.as_slice();
    let topic_keccak_data: Vec<u8> = keccak256_hash(topic_data_vec);
    let topic_keccak_data_vec: &[u8] = topic_keccak_data.as_slice();
    let topic_h256 = H256::from_slice(topic_keccak_data_vec);
    topic_h256
}

pub fn amount_bytes_slice_to_big_decimal(bytes: &[u8]) -> BigDecimal {
    let vec = bytes.to_vec();
    let bit_vec: BitVec = vec.into();
    let mut bool_vec: Vec<bool> = vec![];
    for i in bit_vec {
        bool_vec.push(i);
    }
    let amount_u128: u128 = parse_float_to_u128(
        bool_vec,
        AMOUNT_EXPONENT_BIT_WIDTH,
        AMOUNT_MANTISSA_BIT_WIDTH,
        10
    ).unwrap_or(0);
    let amount_u64 = amount_u128 as u64;
    // amount_f64 = amount_f64 / f64::from(1000000);
    BigDecimal::from(amount_u64)
}

pub fn fee_bytes_slice_to_big_decimal(byte: &u8) -> BigDecimal {
    let bit_vec: BitVec = BitVec::from_element(*byte);
    let mut bool_vec: Vec<bool> = vec![];
    for i in bit_vec {
        bool_vec.push(i);
    }
    let fee_u128: u128 = parse_float_to_u128(
        bool_vec,
        FEE_EXPONENT_BIT_WIDTH,
        FEE_MANTISSA_BIT_WIDTH,
        10
    ).unwrap_or(0);
    let fee_u64 = fee_u128 as u64;
    // fee_f64 = fee_f64 / f64::from(1000000);
    BigDecimal::from(fee_u64)
}

#[derive(Debug, Clone)]
pub enum DataRestoreError {
    Unknown(String),
    WrongType,
    NoData(String),
    NonexistentAccount,
    WrongAmount,
    WrongEndpoint,
    WrongPubKey,
    DoubleExit,
    StateUpdate(String),
}

impl std::string::ToString for DataRestoreError {
    fn to_string(&self) -> String {
        match self {
            DataRestoreError::Unknown(text)      => format!("Unknown {}", text),
            DataRestoreError::WrongType          => "Wrong type".to_owned(),
            DataRestoreError::NoData(text)       => format!("No data {}", text),
            DataRestoreError::NonexistentAccount => "Nonexistent account".to_owned(),
            DataRestoreError::WrongAmount        => "Wrong amount".to_owned(),
            DataRestoreError::WrongEndpoint      => "Wrong endpoint".to_owned(),
            DataRestoreError::WrongPubKey        => "Wrong pubkey".to_owned(),
            DataRestoreError::DoubleExit         => "Double exit".to_owned(),
            DataRestoreError::StateUpdate(text)  => format!("Error during state update {}", text),
        }
    }
}
