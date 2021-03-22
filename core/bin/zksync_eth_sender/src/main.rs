use futures::{channel::mpsc, executor::block_on, SinkExt, StreamExt};
use std::cell::RefCell;
use zksync_config::ZkSyncConfig;
use zksync_eth_sender::run_eth_sender;
use zksync_prometheus_exporter::run_prometheus_exporter;
use zksync_storage::ConnectionPool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // `eth_sender` doesn't require many connections to the database.
    const ETH_SENDER_CONNECTION_POOL_SIZE: u32 = 2;

    let _sentry_guard = vlog::init();

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

    let pool = ConnectionPool::new(Some(ETH_SENDER_CONNECTION_POOL_SIZE));
    let config = ZkSyncConfig::from_env();

    // Run prometheus data exporter.
    let (prometheus_task_handle, _) =
        run_prometheus_exporter(pool.clone(), config.api.prometheus.port, false);

    let task_handle = run_eth_sender(pool, config);

    tokio::select! {
        _ = async { task_handle.await } => {
            panic!("Ethereum sender actors aren't supposed to finish their execution")
        },
        _ = async { prometheus_task_handle.await } => {
            panic!("Prometheus exporter actors aren't supposed to finish their execution")
        },
        _ = async { stop_signal_receiver.next().await } => {
            vlog::warn!("Stop signal received, shutting down");
        }
    };

    Ok(())
}
