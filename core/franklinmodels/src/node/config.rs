use lazy_static::lazy_static;

pub const TRANSFER_BATCH_SIZE: usize = 8;
pub const DEPOSIT_BATCH_SIZE: usize = 1;
pub const EXIT_BATCH_SIZE: usize = 1;
pub const PADDING_INTERVAL: u64 = 5; // sec
pub const PROVER_TIMEOUT: usize = 60; // sec
pub const PROVER_TIMER_TICK: u64 = 5; // sec
pub const PROVER_CYCLE_WAIT: u64 = 5; // sec

pub const DEFAULT_KEYS_PATH: &str = "keys";

lazy_static! {
    pub static ref RUNTIME_CONFIG: RuntimeConfig = RuntimeConfig::new();
}

use std::env;

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub transfer_batch_size: usize,
    pub keys_path: String,
    pub max_outstanding_txs: u32,
    pub contract_addr: String,
    pub mainnet_http_endpoint_string: String,
    pub rinkeby_http_endpoint_string: String,
    pub mainnet_franklin_contract_address: String,
    pub rinkeby_franklin_contract_address: String,
}

impl RuntimeConfig {
    pub fn new() -> Self {
        let transfer_batch_size_env =
            env::var("TRANSFER_BATCH_SIZE").expect("TRANSFER_BATCH_SIZE env missing");
        let transfer_size = usize::from_str_radix(&(transfer_batch_size_env), 10)
            .expect("TRANSFER_BATCH_SIZE invalid");
        let keys_path = env::var("KEY_DIR")
            .ok()
            .unwrap_or_else(|| DEFAULT_KEYS_PATH.to_string());

        Self {
            transfer_batch_size: transfer_size,
            keys_path,
            contract_addr: env::var("CONTRACT_ADDR").expect("CONTRACT_ADDR env missing"),
            max_outstanding_txs: env::var("MAX_OUTSTANDING_TXS")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .expect("MAX_OUTSTANDING_TXS env var not set"),
            mainnet_http_endpoint_string: env::var("TREE_RESTORE_MAINNET_ENDPOINT")
                .expect("TREE_RESTORE_MAINNET_ENDPOINT env missing"),
            rinkeby_http_endpoint_string: env::var("TREE_RESTORE_RINKEBY_ENDPOINT")
                .expect("TREE_RESTORE_RINKEBY_ENDPOINT env missing"),
            mainnet_franklin_contract_address: env::var("TREE_RESTORE_MAINNET_CONTRACT_ADDR")
                .expect("TREE_RESTORE_MAINNET_CONTRACT_ADDR env missing"),
            rinkeby_franklin_contract_address: env::var("TREE_RESTORE_RINKEBY_CONTRACT_ADDR")
                .expect("TREE_RESTORE_RINKEBY_CONTRACT_ADDR env missing"),
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self::new()
    }
}
