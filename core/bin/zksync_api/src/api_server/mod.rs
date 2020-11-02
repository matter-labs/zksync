//! API server handles endpoints for interaction with node.
//!
//! `mod rest` - api is used for block explorer.
//! `mod rpc_server` - JSON rpc via HTTP (for request reply functions)
//! `mod rpc_subscriptions` - JSON rpc via WebSocket (for request reply functions and subscriptions)

// External uses
use futures::channel::mpsc;
// Workspace uses
use zksync_config::{AdminServerOptions, ConfigurationOptions};
use zksync_storage::ConnectionPool;
// Local uses
use crate::fee_ticker::TickerRequest;
use crate::signature_checker;

mod admin_server;
mod event_notify;
mod loggers;
mod rest;
pub mod rpc_server;
mod rpc_subscriptions;

/// Amount of threads used by each server to serve requests.
const THREADS_PER_SERVER: usize = 128;

#[allow(clippy::too_many_arguments)]
pub fn start_api_server(
    connection_pool: ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
    ticker_request_sender: mpsc::Sender<TickerRequest>,
    config_options: ConfigurationOptions,
    admin_server_opts: AdminServerOptions,
) {
    let (sign_check_sender, sign_check_receiver) = mpsc::channel(8192);

    signature_checker::start_sign_checker_detached(
        config_options.clone(),
        sign_check_receiver,
        panic_notify.clone(),
    );

    rest::start_server_thread_detached(
        connection_pool.clone(),
        config_options.rest_api_server_address,
        config_options.contract_eth_addr,
        panic_notify.clone(),
        config_options.clone(),
    );
    rpc_subscriptions::start_ws_server(
        &config_options,
        connection_pool.clone(),
        sign_check_sender.clone(),
        ticker_request_sender.clone(),
        panic_notify.clone(),
    );

    admin_server::start_admin_server(
        admin_server_opts.admin_http_server_address,
        admin_server_opts.secret_auth,
        connection_pool.clone(),
        panic_notify.clone(),
    );

    rpc_server::start_rpc_server(
        config_options,
        connection_pool,
        sign_check_sender,
        ticker_request_sender,
        panic_notify,
    );
}
