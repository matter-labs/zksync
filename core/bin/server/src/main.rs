use futures::{channel::mpsc, executor::block_on, SinkExt, StreamExt};
use std::cell::RefCell;
use structopt::StructOpt;
use zksync_api::run_api;
use zksync_config::{ConfigurationOptions, ProverOptions};
use zksync_core::{genesis_init, run_core, wait_for_tasks};
use zksync_eth_sender::run_eth_sender;
use zksync_prometheus_exporter::run_prometheus_exporter;
use zksync_witness_generator::run_prover_server;

use zksync_storage::ConnectionPool;

#[derive(Debug, Clone, Copy)]
pub enum ServerCommand {
    Genesis,
    Launch,
}

#[derive(StructOpt)]
#[structopt(name = "zkSync operator node", author = "Matter Labs")]
struct Opt {
    /// Generate genesis block for the first contract deployment
    #[structopt(long)]
    genesis: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let opt = Opt::from_args();

    let server_mode = if opt.genesis {
        ServerCommand::Genesis
    } else {
        ServerCommand::Launch
    };

    if let ServerCommand::Genesis = server_mode {
        log::info!("Performing the server genesis initialization");
        genesis_init().await;
        return Ok(());
    }

    // It's a `ServerCommand::Launch`, perform the usual routine.
    log::info!("Running the zkSync server");

    let connection_pool = ConnectionPool::new(None).await;
    let config_options = ConfigurationOptions::from_env();
    let prover_options = ProverOptions::from_env();

    // Handle Ctrl+C
    let (stop_signal_sender, mut stop_signal_receiver) = mpsc::channel(256);
    {
        let stop_signal_sender = RefCell::new(stop_signal_sender.clone());
        ctrlc::set_handler(move || {
            let mut sender = stop_signal_sender.borrow_mut();
            block_on(sender.send(true)).expect("Ctrl+C signal send");
        })
        .expect("Error setting Ctrl+C handler");
    }

    // Run core actors.
    log::info!("Starting the Core actors");
    let core_task_handles = run_core(connection_pool.clone(), stop_signal_sender.clone())
        .await
        .expect("Unable to start Core actors");

    // Run API actors.
    log::info!("Starting the API server actors");
    let api_task_handle = run_api(connection_pool.clone(), stop_signal_sender.clone());

    // Run Ethereum sender actors.
    log::info!("Starting the Ethereum sender actors");
    let eth_sender_task_handle = run_eth_sender(connection_pool.clone(), config_options.clone());

    // Run prometheus data exporter.
    let prometheus_task_handle = run_prometheus_exporter(connection_pool.clone(), &config_options);

    // Run prover server & witness generator.
    log::info!("Starting the Prover server actors");
    run_prover_server(
        connection_pool,
        stop_signal_sender,
        prover_options,
        config_options,
    );

    tokio::select! {
        _ = async { wait_for_tasks(core_task_handles).await } => {
            // We don't need to do anything here, since Core actors will panic upon future resolving.
        },
        _ = async { api_task_handle.await } => {
            panic!("API server actors aren't supposed to finish their execution")
        },
        _ = async { eth_sender_task_handle.await } => {
            panic!("Ethereum Sender actors aren't supposed to finish their execution")
        },
        _ = async { prometheus_task_handle.await } => {
            panic!("Prometheus actors aren't supposed to finish their execution")
        },
        _ = async { stop_signal_receiver.next().await } => {
            log::warn!("Stop signal received, shutting down");
        }
    };

    Ok(())
}
