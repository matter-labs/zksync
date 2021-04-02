use futures::{channel::mpsc, executor::block_on, SinkExt, StreamExt};
use std::cell::RefCell;
use zksync_config::ZkSyncConfig;
use zksync_core::{run_core, wait_for_tasks};
use zksync_eth_client::EthereumGateway;
use zksync_prometheus_exporter::run_prometheus_exporter;
use zksync_storage::ConnectionPool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _sentry_guard = vlog::init();
    // handle ctrl+c
    let config = ZkSyncConfig::from_env();
    let eth_gateway = EthereumGateway::from_config(&config);
    let (stop_signal_sender, mut stop_signal_receiver) = mpsc::channel(256);
    {
        let stop_signal_sender = RefCell::new(stop_signal_sender.clone());
        ctrlc::set_handler(move || {
            let mut sender = stop_signal_sender.borrow_mut();
            block_on(sender.send(true)).expect("Ctrl+C signal send");
        })
        .expect("Error setting Ctrl+C handler");
    }
    let connection_pool = ConnectionPool::new(None);

    // Run prometheus data exporter.
    let (prometheus_task_handle, counter_task_handle) =
        run_prometheus_exporter(connection_pool.clone(), config.api.prometheus.port, true);

    let task_handles = run_core(connection_pool, stop_signal_sender, eth_gateway, &config)
        .await
        .expect("Unable to start Core actors");

    tokio::select! {
        _ = async { wait_for_tasks(task_handles).await } => {
            // We don't need to do anything here, since actors will panic upon future resolving.
        },
        _ = async { prometheus_task_handle.await } => {
            panic!("Prometheus exporter actors aren't supposed to finish their execution")
        },
        _ = async { counter_task_handle.unwrap().await } => {
            panic!("Operation counting actor is not supposed to finish its execution")
        },
        _ = async { stop_signal_receiver.next().await } => {
            vlog::warn!("Stop signal received, shutting down");
        }
    };

    Ok(())
}
