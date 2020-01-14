// Built-in deps
use std::env;
use std::str::FromStr;
// External deps
use ethabi::Contract;
use failure::format_err;
use lazy_static::lazy_static;
use models::abi::zksync_contract;
use web3::futures::Future;
use web3::types::{Address, H256};
use web3::types::{Transaction, TransactionId};
// Workspace deps

pub const FUNC_NAME_HASH_LENGTH: usize = 4;

lazy_static! {
    pub static ref DATA_RESTORE_CONFIG: DataRestoreConfig = DataRestoreConfig::from_env();
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
    pub fn from_env() -> Self {
        Self {
            web3_endpoint: env::var("WEB3_URL").expect("WEB3_URL env missing"), //"https://rinkeby.infura.io/".to_string(),
            franklin_contract: zksync_contract(),
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

/// Return Ethereum transaction input data
///
/// # Arguments
///
/// * `transaction` - Ethereum transaction description
///
pub fn get_input_data_from_ethereum_transaction(
    transaction: &Transaction,
) -> Result<Vec<u8>, failure::Error> {
    let input_data = transaction.clone().input.0;
    if input_data.len() > FUNC_NAME_HASH_LENGTH {
        Ok(input_data[FUNC_NAME_HASH_LENGTH..input_data.len()].to_vec())
    } else {
        Err(format_err!("No commitment data in tx"))
    }
}

/// Return Ethereum transaction description
///
/// # Arguments
///
/// * `transaction_hash` - The identifier of the particular Ethereum transaction
///
pub fn get_ethereum_transaction(transaction_hash: &H256) -> Result<Transaction, failure::Error> {
    let tx_id = TransactionId::Hash(*transaction_hash);
    let (_eloop, transport) =
        web3::transports::Http::new(DATA_RESTORE_CONFIG.web3_endpoint.as_str())
            .map_err(|e| format_err!("Wrong endpoint: {}", e.to_string()))?;
    let web3 = web3::Web3::new(transport);
    let web3_transaction = web3
        .eth()
        .transaction(tx_id)
        .wait()
        .map_err(|e| format_err!("No response from web3: {}", e.to_string()))?
        .ok_or_else(|| format_err!("No tx with this hash"))?;
    Ok(web3_transaction)
}
