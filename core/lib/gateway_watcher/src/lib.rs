//! Watcher for `EthereumGateway`'s `Multiplexed` variant which checks clients gateways
//! and prioritizes the one with longest chain, most frequent hash and lowest latency.

mod multiplexed_gateway_watcher;
pub use multiplexed_gateway_watcher::{
    run_gateway_watcher_if_multiplexed, run_multiplexed_gateway_watcher, MultiplexedGatewayWatcher,
};
