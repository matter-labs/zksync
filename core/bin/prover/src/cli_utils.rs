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
use crate::{client, prover_work_cycle, ApiClient, ProverConfig, ProverImpl, ShutdownRequest};

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

pub async fn main_for_prover_impl<P: ProverImpl + 'static + Send + Sync>() {
    let opt = Opt::from_args();
    let worker_name = opt.worker_name;

    // used env
    let prover_config = <P as ProverImpl>::Config::from_env();
    let api_client = api_client_from_env(&worker_name);
    let prover = P::create_from_config(prover_config);

    env_logger::init();

    log::info!("creating prover, worker name: {}", worker_name);

    // Create client

    let shutdown_request = ShutdownRequest::new();

    // Handle termination requests.
    {
        let shutdown_request = shutdown_request.clone();
        ctrlc::set_handler(move || {
            log::info!(
                "Termination signal received. It will be handled after the currently working round"
            );

            if shutdown_request.get() {
                log::warn!("Second shutdown request received, shutting down without waiting for round to be completed");
                std::process::exit(0);
            }

            shutdown_request.set();
        })
        .expect("Failed to register ctrlc handler");
    }

    let prover_options = ProverOptions::from_env();
    prover_work_cycle(prover, api_client, shutdown_request, prover_options).await;
}
