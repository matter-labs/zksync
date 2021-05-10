// Public re-exports
pub use self::{
    api::ApiConfig, chain::ChainConfig, contracts::ContractsConfig, database::DBConfig,
    dev_liquidity_token_watcher::DevLiquidityTokenWatcherConfig, eth_client::ETHClientConfig,
    eth_sender::ETHSenderConfig, eth_watch::ETHWatchConfig, event_listener::EventListenerConfig,
    forced_exit_requests::ForcedExitRequestsConfig, gateway_watcher::GatewayWatcherConfig,
    misc::MiscConfig, prover::ProverConfig, ticker::TickerConfig,
    token_handler::TokenHandlerConfig,
};

pub mod api;
pub mod chain;
pub mod contracts;
pub mod database;
pub mod dev_liquidity_token_watcher;
pub mod eth_client;
pub mod eth_sender;
pub mod eth_watch;
pub mod event_listener;
pub mod forced_exit_requests;
pub mod gateway_watcher;
pub mod misc;
pub mod prover;
pub mod ticker;
pub mod token_handler;

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
