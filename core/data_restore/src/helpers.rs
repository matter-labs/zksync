// Built-in uses
use std::env;
use std::str::FromStr;
// External uses
use ethabi::Contract;
use lazy_static::lazy_static;
use serde_json;
use web3::futures::Future;
use web3::types::{Address, H256};
use web3::types::{Transaction, TransactionId};
use failure::format_err;
// Workspace uses
use models::abi::FRANKLIN_CONTRACT;

pub const FUNC_NAME_HASH_LENGTH: usize = 4;

lazy_static! {
    pub static ref NODE_RESTORE_CONFIG: NodeRestoreConfig = NodeRestoreConfig::from_env();
}

/// Configuratoin of NodeRestore driver
#[derive(Debug, Clone)]
pub struct NodeRestoreConfig {
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

impl NodeRestoreConfig {
    /// Return the configuration for setted Infura web3 endpoint
    pub fn from_env() -> Self {
        let get_env =
            |name| env::var(name).unwrap_or_else(|e| panic!("Env var {} missing, {}", name, e));

        let abi_string = serde_json::Value::from_str(FRANKLIN_CONTRACT)
            .expect("Cant get plasma contract")
            .get("abi")
            .expect("Cant get plasma contract abi")
            .to_string();
        Self {
            web3_endpoint: env::var("WEB3_URL").expect("WEB3_URL env missing"), //"https://rinkeby.infura.io/".to_string(),
            franklin_contract: ethabi::Contract::load(abi_string.as_bytes())
                .expect("Cant get plasma contract in node restore config"),

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
        web3::transports::Http::new(NODE_RESTORE_CONFIG.web3_endpoint.as_str())
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
