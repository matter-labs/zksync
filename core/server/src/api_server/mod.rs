//! API server handles endpoints for interaction with node.
//!
//! `mod rest` - api is used for block explorer.
//! `mod rpc_server` - JSON rpc via HTTP (for request reply functions) and WS (also supports subscriptions)

mod event_notify;
mod rest;
mod rpc_server;
mod rpc_subscriptions;

use models::Operation;
use std::sync::mpsc;
use storage::ConnectionPool;

use futures::sync::mpsc as fmpsc;

use std::env;

pub fn start_api_server(
    op_notify_receiver: fmpsc::Receiver<Operation>,
    connection_pool: ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
) {
    let rest_api_listen_addr = env::var("REST_API_BIND")
        .expect("REST_API_BIND not found")
        .parse()
        .expect("REST_API_BIND invalid");
    rest::start_server_thread_detached(
        connection_pool.clone(),
        rest_api_listen_addr,
        panic_notify.clone(),
    );
    let ws_api_listen_addr = env::var("WS_API_BIND")
        .expect("WS_API_BIND not found")
        .parse()
        .expect("WS_API_BIND invalid");
    rpc_subscriptions::start_ws_server(
        op_notify_receiver,
        connection_pool.clone(),
        ws_api_listen_addr,
        panic_notify.clone(),
    );

    let http_rpc_api_listen_addr = env::var("HTTP_RPC_API_BIND")
        .expect("HTTP_RPC_API_BIND not found")
        .parse()
        .expect("HTTP_RPC_API_BIND invalid");
    rpc_server::start_rpc_server(
        http_rpc_api_listen_addr,
        connection_pool.clone(),
        panic_notify.clone(),
    );
}
