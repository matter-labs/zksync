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
use clap::{App, Arg};
// Workspace deps
use models::config_options::{parse_env, ProverOptions};
// Local deps
use crate::{client, start, ProverConfig, ProverImpl};

fn api_client_from_env(worker_name: &str) -> client::ApiClient {
    let server_api_url = parse_env("PROVER_SERVER_URL");
    let request_timout = Duration::from_secs(parse_env::<u64>("REQ_SERVER_TIMEOUT"));
    client::ApiClient::new(&server_api_url, worker_name, request_timout)
}

pub fn main_for_prover_impl<P: ProverImpl<client::ApiClient> + 'static + Send + Sync>() {
    let cli = App::new("Plonk step by step prover")
        .author("Matter Labs")
        .arg(
            Arg::with_name("worker_name")
                .help("Name of the worker. Must be unique!")
                .required(true)
                .index(1),
        )
        .get_matches();
    let worker_name = cli.value_of("worker_name").unwrap();

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

    // Handle termination requests.
    {
        let prover_id_arc = prover_id_arc.clone();
        let api_client = api_client.clone();
        ctrlc::set_handler(move || {
            log::info!("Termination signal received.");
            let prover_id = prover_id_arc.load(Ordering::SeqCst);
            if prover_id != ABSENT_PROVER_ID {
                match api_client.prover_stopped(prover_id) {
                    Ok(_) => {}
                    Err(e) => log::error!("failed to send prover stop request: {}", e),
                }
            }
            std::process::exit(0);
        })
        .expect("Failed to register ctrlc handler");
    }

    // Register prover
    prover_id_arc.store(
        api_client
            .register_prover(0)
            .expect("failed to register prover"),
        Ordering::SeqCst,
    );

    // Start prover
    let (exit_err_tx, exit_err_rx) = mpsc::channel();
    let jh = thread::spawn(move || {
        start(prover, exit_err_tx);
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
