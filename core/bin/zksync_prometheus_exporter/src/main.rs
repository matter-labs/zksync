use futures::{channel::mpsc, executor::block_on, SinkExt, StreamExt};
use std::cell::RefCell;
use zksync_config::configs::ApiConfig;
use zksync_prometheus_exporter::run_prometheus_exporter;
use zksync_storage::ConnectionPool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Prometheus doesn't require many connections to the database.
    const PROMETHEUS_EXPORTER_CONNECTION_POOL_SIZE: u32 = 1;

    vlog::init();

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

    let connection_pool = ConnectionPool::new(Some(PROMETHEUS_EXPORTER_CONNECTION_POOL_SIZE));
    let api_options = ApiConfig::from_env();

    let (prometheus_handle, counter_handle) =
        run_prometheus_exporter(connection_pool, api_options.prometheus.port);

    tokio::select! {
        _ = async { prometheus_handle.await } => {
            panic!("Prometheus exporter actors aren't supposed to finish their execution")
        },
        _ = async { counter_handle.await } => {
            panic!("Operation counting actor is not supposed to finish its execution")
        },
        _ = async { stop_signal_receiver.next().await } => {
            vlog::warn!("Stop signal received, shutting down");
        }
    };

    Ok(())
}
