//! Watcher for `EthereumGateway`'s `Multiplexed` variant which checks clients gateways
//! and prioritizes the one with longest chain with the most frequent hash of its depth.

mod multiplexed_gateway_watcher;
pub use multiplexed_gateway_watcher::{run_multiplexed_gateway_watcher, MultiplexedGatewayWatcher};
