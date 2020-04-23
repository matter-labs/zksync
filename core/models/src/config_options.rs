// Built-in deps
use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;
// External uses
use futures::{channel::mpsc, executor::block_on, SinkExt};
use web3::types::{H160, H256};
// Local uses
use crate::node::Address;

/// If its placed inside thread::spawn closure it will notify channel when this thread panics.
pub struct ThreadPanicNotify(pub mpsc::Sender<bool>);

impl Drop for ThreadPanicNotify {
    fn drop(&mut self) {
        if std::thread::panicking() {
            block_on(self.0.send(true)).unwrap();
        }
    }
}

/// Obtains the environment variable value.
/// Panics if there is no environment variable with provided name set.
pub fn get_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|e| panic!("Env var {} missing, {}", name, e))
}

/// Obtains the environment variable value and parses it using the `FromStr` type implementation.
/// Panics if there is no environment variable with provided name set, or the value cannot be parsed.
pub fn parse_env<F>(name: &str) -> F
where
    F: FromStr,
    F::Err: std::fmt::Debug,
{
    get_env(name)
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse environment variable {}: {:?}", name, e))
}

/// Similar to `parse_env`, but also takes a function to change the variable value before parsing.
pub fn parse_env_with<T, F>(name: &str, f: F) -> T
where
    T: FromStr,
    T::Err: std::fmt::Debug,
    F: FnOnce(&str) -> &str,
{
    let env_var = get_env(name);

    f(&env_var)
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse environment variable {}: {:?}", name, e))
}

/// Configuration options for `eth_sender`.
#[derive(Debug, Clone)]
pub struct EthSenderOptions {
    pub expected_wait_time_block: u64,
    pub tx_poll_period: Duration,
    pub wait_confirmations: u64,
    pub max_txs_in_flight: u64,
}

impl EthSenderOptions {
    /// Parses the `eth_sender` configuration options values from the environment variables.
    /// Panics if any of options is missing or has inappropriate value.
    pub fn from_env() -> Self {
        let tx_poll_period_secs: u64 = parse_env("ETH_TX_POLL_PERIOD");

        Self {
            expected_wait_time_block: parse_env("ETH_EXPECTED_WAIT_TIME_BLOCK"),
            tx_poll_period: Duration::new(tx_poll_period_secs, 0),
            wait_confirmations: parse_env("ETH_WAIT_CONFIRMATIONS"),
            max_txs_in_flight: parse_env("ETH_MAX_TXS_IN_FLIGHT"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigurationOptions {
    pub replica_name: String,
    pub rest_api_server_address: SocketAddr,
    pub json_rpc_http_server_address: SocketAddr,
    pub json_rpc_ws_server_address: SocketAddr,
    pub contract_eth_addr: H160,
    pub contract_genesis_tx_hash: H256,
    pub web3_url: String,
    pub governance_eth_addr: H160,
    pub governance_genesis_tx_hash: H256,
    pub operator_franklin_addr: Address,
    pub operator_eth_addr: H160,
    pub operator_private_key: H256,
    pub chain_id: u8,
    pub gas_price_factor: usize,
    pub tx_batch_size: usize,
    pub prover_server_address: SocketAddr,
    pub confirmations_for_eth_event: u64,
}

impl ConfigurationOptions {
    /// Parses the configuration options values from the environment variables.
    /// Panics if any of options is missing or has inappropriate value.
    pub fn from_env() -> Self {
        Self {
            replica_name: parse_env("SERVER_REPLICA_NAME"),
            rest_api_server_address: parse_env("REST_API_BIND"),
            json_rpc_http_server_address: parse_env("HTTP_RPC_API_BIND"),
            json_rpc_ws_server_address: parse_env("WS_API_BIND"),
            contract_eth_addr: parse_env_with("CONTRACT_ADDR", |s| &s[2..]),
            contract_genesis_tx_hash: parse_env_with("CONTRACT_GENESIS_TX_HASH", |s| &s[2..]),
            web3_url: get_env("WEB3_URL"),
            governance_eth_addr: parse_env_with("GOVERNANCE_ADDR", |s| &s[2..]),
            governance_genesis_tx_hash: parse_env_with("GOVERNANCE_GENESIS_TX_HASH", |s| &s[2..]),
            operator_franklin_addr: parse_env_with("OPERATOR_FRANKLIN_ADDRESS", |s| &s[2..]),
            operator_eth_addr: parse_env_with("OPERATOR_ETH_ADDRESS", |s| &s[2..]),
            operator_private_key: parse_env("OPERATOR_PRIVATE_KEY"),
            chain_id: parse_env("CHAIN_ID"),
            gas_price_factor: parse_env("GAS_PRICE_FACTOR"),
            tx_batch_size: parse_env("TX_BATCH_SIZE"),
            prover_server_address: parse_env("PROVER_SERVER_BIND"),
            confirmations_for_eth_event: parse_env("CONFIRMATIONS_FOR_ETH_EVENT"),
        }
    }
}
