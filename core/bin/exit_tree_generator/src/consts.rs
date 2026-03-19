/// Default filename for the new leaves CSV output file
pub const NEW_LEAVES_CSV: &str = "new_leaves.csv";

/// Default filename for storing tree internals (internal node hashes)
pub const INTERNALS_FILE: &str = "internals.txt";

/// Default filename for the restored tokens CSV output
pub const RESTORED_TOKENS_CSV: &str = "restored_tokens.csv";

/// Number of Ethereum blocks to process in each batch
pub const ETH_BLOCKS_STEP: u64 = 10_000;

/// Maximum number of retry attempts for loading data
pub const MAX_RETRIES: usize = 5;

/// Number of confirmations required before considering Ethereum sync finished
pub const ETH_SYNC_CONFIRMATIONS: u64 = 15;
