// External uses
use serde::Deserialize;
// Local uses
use crate::envy_load;

/// Configuration for the Ethereum sender crate.
#[derive(Debug, Deserialize)]
pub struct ETHWatchConfig {
    /// Amount of confirmations for the priority operation to be processed.
    /// In production this should be a non-zero value because of block reverts.
    pub confirmations_for_eth_event: u64,
    /// How often we want to poll the Ethereum node.
    pub eth_node_poll_interval: u64,
}

impl ETHWatchConfig {
    pub fn from_env() -> Self {
        envy_load!("eth_watch", "ETH_WATCH_")
    }
}
