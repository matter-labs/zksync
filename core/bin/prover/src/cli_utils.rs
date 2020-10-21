// Built-in deps
use std::{
    sync::{
        atomic::{AtomicI32, Ordering},
        mpsc, Arc,
    },
    thread,
    time::Duration,
};
// External deps
use structopt::StructOpt;
// Workspace deps
use zksync_config::ProverOptions;
use zksync_utils::parse_env;
// Local deps
use crate::{client, start, ApiClient, ProverConfig, ProverImpl, ShutdownRequest};

fn api_client_from_env(worker_name: &str) -> client::ApiClient {
    let server_api_url = parse_env("PROVER_SERVER_URL");
    let request_timout = Duration::from_secs(parse_env::<u64>("REQ_SERVER_TIMEOUT"));
    client::ApiClient::new(&server_api_url, worker_name, request_timout)
}

#[derive(StructOpt)]
#[structopt(
    name = "zkSync operator node",
    author = "Matter Labs",
    rename_all = "snake_case"
)]
struct Opt {
    /// Name of the worker. Must be unique!
    #[structopt(index = 1)]
    worker_name: String,
}

pub fn main_for_prover_impl<P: ProverImpl<client::ApiClient> + 'static + Send + Sync>() {
    let opt = Opt::from_args();
    let worker_name = opt.worker_name;

    // used env
    let heartbeat_interval = ProverOptions::from_env().heartbeat_interval;
    let prover_config = <P as ProverImpl<client::ApiClient>>::Config::from_env();
    let api_client = api_client_from_env(&worker_name);
    let prover = P::create_from_config(prover_config, api_client.clone(), heartbeat_interval);

    env_logger::init();
    const ABSENT_PROVER_ID: i32 = -1;

    log::info!("creating prover, worker name: {}", worker_name);

    // Create client

    let prover_id_arc = Arc::new(AtomicI32::new(ABSENT_PROVER_ID));

    let shutdown_request = ShutdownRequest::new();

    // Handle termination requests.
    {
        let shutdown_request = shutdown_request.clone();
        ctrlc::set_handler(move || {
            log::info!(
                "Termination signal received. It will be handled after the currently working round"
            );

            if shutdown_request.prover_id() == ABSENT_PROVER_ID {
                log::warn!("Prover is not registered, shutting down immediately");
                std::process::exit(0);
            }

            if shutdown_request.get() {
                log::warn!("Second shutdown request received, shutting down without waiting for round to be completed");
                std::process::exit(0);
            }

            shutdown_request.set();
        })
        .expect("Failed to register ctrlc handler");
    }

    // Register prover
    let prover_id = api_client
        .register_prover(0)
        .expect("failed to register prover");
    shutdown_request.set_prover_id(prover_id);

    // Start prover
    let (exit_err_tx, exit_err_rx) = mpsc::channel();
    let jh = thread::spawn(move || {
        start(prover, exit_err_tx, shutdown_request);
    });

    // Handle prover exit errors.
    let err = exit_err_rx.recv();
    jh.join().expect("failed to join on worker thread");
    log::error!("prover exited with error: {:?}", err);
    {
        let prover_id = prover_id_arc.load(Ordering::SeqCst);
        if prover_id != ABSENT_PROVER_ID {
            match api_client.prover_stopped(prover_id) {
                Ok(_) => {}
                Err(e) => log::error!("failed to send prover stop request: {}", e),
            }
        }
    }
}
