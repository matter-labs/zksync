// Built-in deps
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
// External deps
use clap::{App, Arg};
use log::*;
// Workspace deps
use models::config_options::parse_env;
use models::node::config::PROVER_HEARTBEAT_INTERVAL;
use prover::client;
use prover::{start, BabyProver};
use std::time::Duration;

fn main() {
    let cli = App::new("Prover")
        .author("Matter Labs")
        .arg(
            Arg::with_name("worker_name")
                .help("Name of the worker. Must be unique!")
                .required(true)
                .index(1),
        )
        .get_matches();

    let worker_name = cli.value_of("worker_name").unwrap();

    env_logger::init();
    const ABSENT_PROVER_ID: i32 = -1;

    info!("creating prover, worker name: {}", worker_name);

    // Create client
    let api_client = {
        let api_url = parse_env("PROVER_SERVER_URL");
        let req_server_timeout = Duration::from_secs(parse_env::<u64>("REQ_SERVER_TIMEOUT"));
        client::ApiClient::new(&api_url, &worker_name, req_server_timeout)
    };
    let prover_id_arc = Arc::new(AtomicI32::new(ABSENT_PROVER_ID));

    // Handle termination requests.
    {
        let prover_id_arc = prover_id_arc.clone();
        let api_client = api_client.clone();
        ctrlc::set_handler(move || {
            info!("Termination signal received.");
            let prover_id = prover_id_arc.load(Ordering::SeqCst);
            if prover_id != ABSENT_PROVER_ID {
                match api_client.prover_stopped(prover_id) {
                    Ok(_) => {}
                    Err(e) => error!("failed to send prover stop request: {}", e),
                }
            }
            std::process::exit(0);
        })
        .expect("Failed to register ctrlc handler");
    }

    let heartbeat_interval = PROVER_HEARTBEAT_INTERVAL;
    let worker = BabyProver::new(
        vec![8], // TODO: tmp
        api_client.clone(),
        heartbeat_interval,
    );

    // Register prover
    prover_id_arc.store(
        api_client
            .register_prover(8) //TODO: tmp
            .expect("failed to register prover"),
        Ordering::SeqCst,
    );

    // Start prover
    let (exit_err_tx, exit_err_rx) = mpsc::channel();
    let jh = thread::spawn(move || {
        start(worker, exit_err_tx);
    });

    // Handle prover exit errors.
    let err = exit_err_rx.recv();
    jh.join().expect("failed to join on worker thread");
    error!("prover exited with error: {:?}", err);
    {
        let prover_id = prover_id_arc.load(Ordering::SeqCst);
        if prover_id != ABSENT_PROVER_ID {
            match api_client.prover_stopped(prover_id) {
                Ok(_) => {}
                Err(e) => error!("failed to send prover stop request: {}", e),
            }
        }
    }
}
