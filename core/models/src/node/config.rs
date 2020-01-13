use std::time::Duration;

pub const PADDING_SUB_INTERVAL: u64 = 10; // sec
pub const BLOCK_FORMATION_TRIES: usize = 1;
pub const PROVER_GONE_TIMEOUT: usize = 60; // sec
pub const PROVER_PREPARE_DATA_INTERVAL: u64 = 10; // sec
pub const PROVER_HEARTBEAT_INTERVAL: u64 = 5; // sec
pub const PROVER_CYCLE_WAIT: u64 = 5; // sec
pub const TX_MINIBATCH_CREATE_TIME: Duration = Duration::from_millis(100);

pub const DEFAULT_KEYS_PATH: &str = "keys";
