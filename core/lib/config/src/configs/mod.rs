// Public re-exports
pub use self::{
    api::ApiConfig, chain::ChainConfig, contracts::ContractsConfig, database::DBConfig,
    dev_liquidity_token_watcher::DevLiquidityTokenWatcherConfig, eth_client::ETHClientConfig,
    eth_sender::ETHSenderConfig, eth_watch::ETHWatchConfig, misc::MiscConfig, prover::ProverConfig,
    ticker::TickerConfig,
};

pub mod api;
pub mod chain;
pub mod contracts;
pub mod database;
pub mod dev_liquidity_token_watcher;
pub mod eth_client;
pub mod eth_sender;
pub mod eth_watch;
pub mod misc;
pub mod prover;
pub mod ticker;

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
