pub const TRANSFER_BATCH_SIZE: usize = 8;
pub const DEPOSIT_BATCH_SIZE: usize = 1;
pub const EXIT_BATCH_SIZE: usize = 1;
pub const PADDING_INTERVAL: u64 = 1*60; // 1 min
pub const PROVER_TIMEOUT: usize = 20*60; // 20 min
pub const DEFAULT_KEYS_PATH: &str = "keys";

lazy_static! {
    pub static ref RUNTIME_CONFIG: RuntimeConfig = RuntimeConfig::new();
}

pub struct RuntimeConfig {
    pub transfer_batch_size: usize,
    pub keys_path: String,
}

impl RuntimeConfig {
    fn new() -> Self {
        let mut transfer_size = TRANSFER_BATCH_SIZE;
        {
            let transfer_batch_size_env = std::env::var("TRANSFER_BATCH_SIZE");
            if transfer_batch_size_env.is_ok() {
                transfer_size = usize::from_str_radix(&(transfer_batch_size_env.unwrap()), 10).ok().unwrap_or(TRANSFER_BATCH_SIZE);
            }
        }

        let keys_path = std::env::var("KEYS_DIR").ok().unwrap_or(DEFAULT_KEYS_PATH.to_string());

        Self {
            transfer_batch_size:    transfer_size,
            keys_path:              keys_path
        }
    }
}