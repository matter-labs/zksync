#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;

use models::node::AccountAddress;
use std::env;
use std::net::SocketAddr;
use web3::types::{H160, H256};

pub mod api_server;
pub mod committer;
pub mod eth_sender;
pub mod eth_watch;
pub mod state_keeper;

/// If its placed inside thread::spawn closure it will notify channel when this thread panics.
pub struct ThreadPanicNotify(pub std::sync::mpsc::Sender<bool>);

impl Drop for ThreadPanicNotify {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.send(true).unwrap();
        }
    }
}

#[derive(Clone)]
pub struct ConfigurationOptions {
    pub rest_api_server_address: SocketAddr,
    pub json_rpc_http_server_address: SocketAddr,
    pub json_rpc_ws_server_address: SocketAddr,
    pub contract_eth_addr: H160,
    pub web3_url: String,
    pub governance_eth_addr: H160,
    pub priority_queue_eth_addr: H160,
    pub operator_franklin_addr: AccountAddress,
    pub operator_eth_addr: H160,
    pub operator_private_key: H256,
    pub chain_id: u8,
    pub gas_price_factor: usize,
    pub tx_batch_size: usize,
}

impl ConfigurationOptions {
    pub fn from_env() -> ConfigurationOptions {
        let get_env =
            |name| env::var(name).unwrap_or_else(|e| panic!("Env var {} missing, {}", name, e));

        ConfigurationOptions {
            rest_api_server_address: get_env("REST_API_BIND")
                .parse()
                .expect("Failed to parse REST_API_BIND bind address"),
            json_rpc_http_server_address: get_env("HTTP_RPC_API_BIND")
                .parse()
                .expect("Failed to parse HTTP_RPC_API_BIND bind address"),
            json_rpc_ws_server_address: get_env("WS_API_BIND")
                .parse()
                .expect("Failed to parse WS_API_BIND bind address"),
            contract_eth_addr: get_env("CONTRACT_ADDR")[2..]
                .parse()
                .expect("Failed to parse CONTRACT_ADDR as ETH contract address"),
            web3_url: get_env("WEB3_URL"),
            governance_eth_addr: get_env("GOVERNANCE_ADDR")[2..]
                .parse()
                .expect("Failed to parse GOVERNANCE_ADDR as ETH contract address"),
            priority_queue_eth_addr: get_env("PRIORITY_QUEUE_ADDR")[2..]
                .parse()
                .expect("Failed to parse PRIORITY_QUEUE_ADDR as ETH contract address"),
            operator_franklin_addr: AccountAddress::from_hex(&get_env("OPERATOR_FRANKLIN_ADDRESS"))
                .expect("Failed to parse OPERATOR_FRANKLIN_ADDRESS"),
            operator_eth_addr: get_env("OPERATOR_ETH_ADDRESS")[2..]
                .parse()
                .expect("Failed to parse OPERATOR_ETH_ADDRESS as ETH contract address"),
            operator_private_key: get_env("OPERATOR_PRIVATE_KEY")
                .parse()
                .expect("Failed to parse OPERATOR_ETH_ADDRESS"),
            chain_id: get_env("CHAIN_ID").parse().expect("CHAIN_ID invalid value"),
            gas_price_factor: get_env("GAS_PRICE_FACTOR")
                .parse()
                .expect("gas price factor invalid"),
            tx_batch_size: get_env("TX_BATCH_SIZE")
                .parse()
                .expect("TX_BATCH_SIZE invalid value"),
        }
    }
}
