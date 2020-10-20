#![recursion_limit = "256"]

use crate::{api_server::start_api_server, fee_ticker::run_ticker_task};
use futures::channel::mpsc;
use zksync_config::{AdminServerOptions, ConfigurationOptions};
use zksync_storage::ConnectionPool;

pub mod api_server;
pub mod core_api_client;
pub mod eth_checker;
pub mod fee_ticker;
pub mod signature_checker;
pub mod tx_error;
pub mod utils;

/// Runs the application actors.
pub fn run_api(
    connection_pool: ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
) -> tokio::task::JoinHandle<()> {
    let channel_size = 32768;
    let (ticker_request_sender, ticker_request_receiver) = mpsc::channel(channel_size);

    let config_options = ConfigurationOptions::from_env();
    let admin_server_options = AdminServerOptions::from_env();

    let ticker_task = run_ticker_task(
        config_options.token_price_source.clone(),
        config_options.ticker_fast_processing_coeff,
        connection_pool.clone(),
        ticker_request_receiver,
    );

    start_api_server(
        connection_pool,
        panic_notify,
        ticker_request_sender,
        config_options,
        admin_server_options,
    );

    ticker_task
}
