//! API server handles endpoints for interaction with node.
//!
//! `mod rest` - api is used for block explorer.
//! `mod rpc_server` - JSON rpc via HTTP (for request reply functions)
//! `mod rpc_subscriptions` - JSON rpc via WebSocket (for request reply functions and subscriptions)

mod event_notify;
mod rest;
mod rpc_server;
mod rpc_subscriptions;

use crate::ConfigurationOptions;
use futures::channel::mpsc;
use models::Operation;
use storage::ConnectionPool;

pub fn start_api_server(
    op_notify_receiver: mpsc::Receiver<Operation>,
    connection_pool: ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
    config_options: ConfigurationOptions,
) {
    rest::start_server_thread_detached(
        connection_pool.clone(),
        config_options.rest_api_server_address,
        config_options.contract_eth_addr,
        panic_notify.clone(),
    );
    rpc_subscriptions::start_ws_server(
        op_notify_receiver,
        connection_pool.clone(),
        config_options.json_rpc_ws_server_address,
        panic_notify.clone(),
    );

    rpc_server::start_rpc_server(
        config_options.json_rpc_http_server_address,
        connection_pool.clone(),
        panic_notify.clone(),
    );
}
