// Built-in deps
use std::time::Duration;
// External deps
use structopt::StructOpt;
// Workspace deps
use zksync_config::configs::ProverConfig as EnvProverConfig;
use zksync_utils::{get_env, parse_env};
// Local deps
use crate::{client, prover_work_cycle, ProverConfig, ProverImpl, ShutdownRequest};

fn api_client_from_env() -> client::ApiClient {
    let server_api_url = parse_env("API_PROVER_URL");
    let request_timout = Duration::from_secs(parse_env::<u64>("PROVER_PROVER_REQUEST_TIMEOUT"));
    let secret = get_env("API_PROVER_SECRET_AUTH");
    client::ApiClient::new(&server_api_url, request_timout, &secret)
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

pub async fn main_for_prover_impl<PROVER>()
where
    PROVER: ProverImpl + Send + Sync + 'static,
{
    let opt = Opt::from_args();
    let worker_name = opt.worker_name;

    // used env
    let prover_options = EnvProverConfig::from_env();
    let prover_config = <PROVER as ProverImpl>::Config::from_env();
    let api_client = api_client_from_env();
    let prover = PROVER::create_from_config(prover_config);

    let _sentry_guard = vlog::init();

    vlog::info!("creating prover, worker name: {}", worker_name);

    // Create client.

    let shutdown_request = ShutdownRequest::new();

    // Handle termination requests.
    {
        let shutdown_request = shutdown_request.clone();
        ctrlc::set_handler(move || {
            vlog::info!(
                "Termination signal received. It will be handled after the currently working round"
            );

            if shutdown_request.get() {
                vlog::warn!("Second shutdown request received, shutting down without waiting for round to be completed");
                std::process::exit(0);
            }

            shutdown_request.set();
        })
        .expect("Failed to register ctrlc handler");
    }

    prover_work_cycle(
        prover,
        api_client,
        shutdown_request,
        prover_options,
        &worker_name,
    )
    .await;
}
