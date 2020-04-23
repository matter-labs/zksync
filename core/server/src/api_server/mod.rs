//! API server handles endpoints for interaction with node.
//!
//! `mod rest` - api is used for block explorer.
//! `mod rpc_server` - JSON rpc via HTTP (for request reply functions)
//! `mod rpc_subscriptions` - JSON rpc via WebSocket (for request reply functions and subscriptions)

// External uses
use futures::channel::mpsc;
// Workspace uses
use models::{config_options::ConfigurationOptions, Operation};
use storage::ConnectionPool;
// Local uses
use crate::{
    eth_watch::EthWatchRequest,
    mempool::MempoolRequest,
    signature_checker,
    state_keeper::{ExecutedOpsNotify, StateKeeperRequest},
};

mod event_notify;
mod rest;
pub mod rpc_server;
mod rpc_subscriptions;

#[allow(clippy::too_many_arguments)]
pub fn start_api_server(
    op_notify_receiver: mpsc::Receiver<Operation>,
    connection_pool: ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
    mempool_request_sender: mpsc::Sender<MempoolRequest>,
    executed_tx_receiver: mpsc::Receiver<ExecutedOpsNotify>,
    state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
    eth_watch_req: mpsc::Sender<EthWatchRequest>,
    config_options: ConfigurationOptions,
) {
    let (sign_check_sender, sign_check_receiver) = mpsc::channel(8192);

    signature_checker::start_sign_checker_detached(
        sign_check_receiver,
        eth_watch_req,
        panic_notify.clone(),
    );

    rest::start_server_thread_detached(
        connection_pool.clone(),
        config_options.rest_api_server_address,
        config_options.contract_eth_addr,
        mempool_request_sender.clone(),
        panic_notify.clone(),
        config_options.api_requests_caches_size,
    );
    rpc_subscriptions::start_ws_server(
        op_notify_receiver,
        connection_pool.clone(),
        config_options.json_rpc_ws_server_address,
        mempool_request_sender.clone(),
        executed_tx_receiver,
        state_keeper_request_sender.clone(),
        sign_check_sender.clone(),
        panic_notify.clone(),
        config_options.api_requests_caches_size,
    );

    rpc_server::start_rpc_server(
        config_options.json_rpc_http_server_address,
        connection_pool,
        mempool_request_sender,
        state_keeper_request_sender,
        sign_check_sender,
        panic_notify,
        config_options.api_requests_caches_size,
    );
}
