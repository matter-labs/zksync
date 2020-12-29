// External deps
use serde::Deserialize;

// Public re-exports
pub use self::{
    api::ApiConfig, chain::ChainConfig, contracts::ContractsConfig, db::DBConfig,
    eth_client::ETHClientConfig, eth_sender::ETHSenderConfig, eth_watch::ETHWatchConfig,
    misc::MiscConfig, prover::ProverConfig, ticker::TickerConfig,
};

pub mod api;
pub mod chain;
pub mod contracts;
pub mod db;
pub mod eth_client;
pub mod eth_sender;
pub mod eth_watch;
pub mod misc;
pub mod prover;
pub mod ticker;

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ZkSyncConfig {
    pub api: ApiConfig,
    pub chain: ChainConfig,
    pub contracts: ContractsConfig,
    pub db: DBConfig,
    pub eth_client: ETHClientConfig,
    pub eth_sender: ETHSenderConfig,
    pub eth_watch: ETHWatchConfig,
    pub prover: ProverConfig,
    pub ticker: TickerConfig,
}

impl ZkSyncConfig {
    pub fn from_env() -> Self {
        Self {
            api: ApiConfig::from_env(),
            chain: ChainConfig::from_env(),
            contracts: ContractsConfig::from_env(),
            db: DBConfig::from_env(),
            eth_client: ETHClientConfig::from_env(),
            eth_sender: ETHSenderConfig::from_env(),
            eth_watch: ETHWatchConfig::from_env(),
            prover: ProverConfig::from_env(),
            ticker: TickerConfig::from_env(),
        }
    }
}

#[cfg(test)]
pub(crate) mod test_utils;

/// Convenience macro that loads the structure from the environment variable given the prefix.
///
/// # Panics
///
/// Panics if the config cannot be loaded from the environment variables.
#[macro_export]
macro_rules! envy_load {
    ($name:expr, $prefix:expr) => {
        envy::prefixed($prefix)
            .from_env()
            .unwrap_or_else(|err| panic!("Cannot load config <{}>: {}", $name, err))
    };
}
