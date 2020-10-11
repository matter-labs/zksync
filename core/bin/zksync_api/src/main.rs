use futures::{channel::mpsc, executor::block_on, SinkExt, StreamExt};
use std::cell::RefCell;
use zksync_api::{api_server::start_api_server, fee_ticker::run_ticker_task};
use zksync_config::{AdminServerOptions, ConfigurationOptions};
use zksync_storage::ConnectionPool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // handle ctrl+c
    let (stop_signal_sender, mut stop_signal_receiver) = mpsc::channel(256);
    {
        let stop_signal_sender = RefCell::new(stop_signal_sender.clone());
        ctrlc::set_handler(move || {
            let mut sender = stop_signal_sender.borrow_mut();
            block_on(sender.send(true)).expect("crtlc signal send");
        })
        .expect("Error setting Ctrl-C handler");
    }

    let channel_size = 32768;
    let (ticker_request_sender, ticker_request_receiver) = mpsc::channel(channel_size);

    // API server requires many connections to the database.
    let connection_pool = ConnectionPool::new(None).await;
    let config_options = ConfigurationOptions::from_env();
    let admin_server_options = AdminServerOptions::from_env();

    let ticker_task = run_ticker_task(
        config_options.token_price_source.clone(),
        config_options.ticker_fast_processing_coeff,
        connection_pool.clone(),
        ticker_request_receiver,
    );

    start_api_server(
        connection_pool.clone(),
        stop_signal_sender.clone(),
        ticker_request_sender,
        config_options,
        admin_server_options.clone(),
    );

    // TODO: Select between `ticker_task` and `stop_signal_receiver`

    stop_signal_receiver.next().await;

    log::info!("Stop signal received, stopping execution");

    Ok(())
}
