//! API server handles endpoints for interaction with node.
//!
//! `mod rest` - api is used for block explorer.
//! `mod rpc_server` - JSON rpc via HTTP (for request reply functions)
//! `mod rpc_subscriptions` - JSON rpc via WebSocket (for request reply functions and subscriptions)

mod event_notify;
mod rest;
mod rpc_server;
mod rpc_subscriptions;

use crate::eth_watch::EthWatchRequest;
use crate::mempool::MempoolRequest;
use crate::state_keeper::{ExecutedOpsNotify, StateKeeperRequest};
use futures::channel::mpsc;
use models::config_options::ConfigurationOptions;
use models::Operation;
use storage::ConnectionPool;

pub fn start_api_server(
    op_notify_receiver: mpsc::Receiver<Operation>,
    connection_pool: ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
    mempool_request_sender: mpsc::Sender<MempoolRequest>,
    executed_tx_receiver: mpsc::Receiver<ExecutedOpsNotify>,
    state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
    eth_watcher_request_sender: mpsc::Sender<EthWatchRequest>,
    config_options: ConfigurationOptions,
) {
    rest::start_server_thread_detached(
        connection_pool.clone(),
        config_options.rest_api_server_address,
        config_options.contract_eth_addr,
        mempool_request_sender.clone(),
        panic_notify.clone(),
    );
    rpc_subscriptions::start_ws_server(
        &config_options,
        op_notify_receiver,
        connection_pool.clone(),
        mempool_request_sender.clone(),
        executed_tx_receiver,
        state_keeper_request_sender.clone(),
        eth_watcher_request_sender.clone(),
        panic_notify.clone(),
    );

    rpc_server::start_rpc_server(
        &config_options,
        connection_pool,
        mempool_request_sender,
        state_keeper_request_sender,
        eth_watcher_request_sender,
        panic_notify,
    );
}
