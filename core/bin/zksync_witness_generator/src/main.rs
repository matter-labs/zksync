use futures::{channel::mpsc, executor::block_on, SinkExt, StreamExt};
use std::cell::RefCell;
use zksync_config::ZkSyncConfig;
use zksync_prometheus_exporter::run_prometheus_exporter;
use zksync_storage::ConnectionPool;
use zksync_witness_generator::database::Database;
use zksync_witness_generator::run_prover_server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // `witness_generator` doesn't require many connections to the database.
    const WITNESS_GENERATOR_CONNECTION_POOL_SIZE: u32 = 2;

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

    let connection_pool = ConnectionPool::new(Some(WITNESS_GENERATOR_CONNECTION_POOL_SIZE));
    let database = Database::new(connection_pool.clone());
    let zksync_config = ZkSyncConfig::from_env();

    // Run prometheus data exporter.
    let (prometheus_task_handle, _) =
        run_prometheus_exporter(connection_pool, zksync_config.api.prometheus.port, false);

    run_prover_server(database, stop_signal_sender, zksync_config);

    tokio::select! {
        _ = async { prometheus_task_handle.await } => {
            panic!("Prometheus exporter actors aren't supposed to finish their execution")
        },
        _ = async { stop_signal_receiver.next().await } => {
            vlog::warn!("Stop signal received, shutting down");
        }
    };

    Ok(())
}
