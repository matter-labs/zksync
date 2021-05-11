//! API server handles endpoints for interaction with node.
//!
//! `mod rest` - api is used for block explorer.
//! `mod rpc_server` - JSON rpc via HTTP (for request reply functions)
//! `mod rpc_subscriptions` - JSON rpc via WebSocket (for request reply functions and subscriptions)

// Public uses
pub use rest::v1;

// External uses
use futures::channel::mpsc;
// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_eth_client::EthereumGateway;
use zksync_storage::ConnectionPool;
// Local uses
use crate::fee_ticker::TickerRequest;
use crate::signature_checker;

mod admin_server;
mod event_notify;
pub mod forced_exit_checker;
mod helpers;
mod rest;
pub mod rpc_server;
mod rpc_subscriptions;
mod tx_sender;

/// Amount of threads used by each server to serve requests.
const THREADS_PER_SERVER: usize = 128;

#[allow(clippy::too_many_arguments)]
pub fn start_api_server(
    connection_pool: ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
    ticker_request_sender: mpsc::Sender<TickerRequest>,
    eth_gateway: EthereumGateway,
    config: &ZkSyncConfig,
) {
    let (sign_check_sender, sign_check_receiver) = mpsc::channel(32768);

    signature_checker::start_sign_checker_detached(
        eth_gateway,
        sign_check_receiver,
        panic_notify.clone(),
    );

    rest::start_server_thread_detached(
        connection_pool.clone(),
        config.api.rest.bind_addr(),
        config.contracts.contract_addr,
        panic_notify.clone(),
        ticker_request_sender.clone(),
        sign_check_sender.clone(),
        config.clone(),
    );

    rpc_subscriptions::start_ws_server(
        connection_pool.clone(),
        sign_check_sender.clone(),
        ticker_request_sender.clone(),
        panic_notify.clone(),
        config,
    );

    admin_server::start_admin_server(
        config.api.admin.bind_addr(),
        config.api.admin.secret_auth.clone(),
        connection_pool.clone(),
        panic_notify.clone(),
    );

    rpc_server::start_rpc_server(
        connection_pool,
        sign_check_sender,
        ticker_request_sender,
        panic_notify,
        config,
    );
}
