use futures::{channel::mpsc, executor::block_on, SinkExt, StreamExt};
use std::cell::RefCell;
use zksync_api::run_api;
use zksync_config::{ConfigurationOptions, ProverOptions};
use zksync_core::{genesis_init, run_core, wait_for_tasks};
use zksync_eth_sender::run_eth_sender;
use zksync_witness_generator::run_prover_server;

use zksync_storage::ConnectionPool;

#[derive(Debug, Clone, Copy)]
pub enum ServerCommand {
    Genesis,
    Launch,
}

fn read_cli() -> ServerCommand {
    let cli = clap::App::new("zkSync operator node")
        .author("Matter Labs")
        .arg(
            clap::Arg::with_name("genesis")
                .long("genesis")
                .help("Generate genesis block for the first contract deployment"),
        )
        .get_matches();

    if cli.is_present("genesis") {
        ServerCommand::Genesis
    } else {
        ServerCommand::Launch
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server_mode = read_cli();

    if let ServerCommand::Launch = server_mode {
        genesis_init().await;
        return Ok(());
    }

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
    let core_task_handles = run_core(connection_pool.clone(), stop_signal_sender.clone())
        .await
        .expect("Unable to start Core actors");

    // Run API actors.
    let api_task_handle = run_api(connection_pool.clone(), stop_signal_sender.clone());

    // Run Ethereum sender actors.
    let eth_sender_task_handle = run_eth_sender(connection_pool.clone(), config_options.clone());

    // Run prover server & witness generator.
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
        _ = async { stop_signal_receiver.next().await } => {
            log::warn!("Stop signal received, shutting down");
        }
    };

    Ok(())
}
