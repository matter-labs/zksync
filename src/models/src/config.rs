pub const TRANSFER_BATCH_SIZE: usize = 8;
pub const DEPOSIT_BATCH_SIZE: usize = 1;
pub const EXIT_BATCH_SIZE: usize = 1;
pub const PADDING_INTERVAL: u64 = 1*60; // 1 min
pub const PROVER_TIMEOUT: usize = 15; // sec
pub const DEFAULT_KEYS_PATH: &str = "keys";

lazy_static! {
    pub static ref RUNTIME_CONFIG: RuntimeConfig = RuntimeConfig::new();
}

use std::env;

pub struct RuntimeConfig {
    pub transfer_batch_size:    usize,
    pub keys_path:              String,
    pub max_outstanding_txs:    u32,
    pub contract_addr:          String,
}

impl RuntimeConfig {
    fn new() -> Self {
        let transfer_batch_size_env = env::var("TRANSFER_BATCH_SIZE").expect("TRANSFER_BATCH_SIZE env missing");
        let transfer_size = usize::from_str_radix(&(transfer_batch_size_env), 10).ok().expect("TRANSFER_BATCH_SIZE invalid");
        let keys_path = env::var("KEY_DIR").ok().unwrap_or(DEFAULT_KEYS_PATH.to_string());

        Self {
            transfer_batch_size:    transfer_size,
            keys_path:              keys_path,
            contract_addr:          env::var("CONTRACT_ADDR").expect("CONTRACT_ADDR env missing"),
            max_outstanding_txs:    env::var("MAX_OUTSTANDING_TXS").ok()
                                    .and_then(|v| v.parse::<u32>().ok())
                                    .expect("MAX_OUTSTANDING_TXS env var not set"),
        }
    }
}