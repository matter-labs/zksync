// Built-in deps
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::{env, thread, time};
// External deps
use clap::{App, Arg};
use crypto_exports::franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use log::*;
use signal_hook::iterator::Signals;
// Workspace deps
use models::node::config::PROVER_HEARTBEAT_INTERVAL;
use prover::{client, read_circuit_params};
use prover::{start, BabyProver};

fn main() {
    let cli = App::new("Prover")
        .author("Matter Labs")
        .arg(
            Arg::with_name("block_size_chunks")
                .help("Size of blocks this prover deals with.")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("worker_name")
                .help("Name of the worker. Must be unique!")
                .required(true)
                .index(2),
        )
        .get_matches();

    let block_size_chunks = usize::from_str(cli.value_of("block_size_chunks").unwrap())
        .expect("invalid block_size_chunks");
    let worker_name = cli.value_of("worker_name").unwrap();

    env_logger::init();
    const ABSENT_PROVER_ID: i32 = -1;

    // handle ctrl+c
    let stop_signal = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::SIGTERM, Arc::clone(&stop_signal))
        .expect("Error setting SIGTERM handler");
    signal_hook::flag::register(signal_hook::SIGINT, Arc::clone(&stop_signal))
        .expect("Error setting SIGINT handler");
    signal_hook::flag::register(signal_hook::SIGQUIT, Arc::clone(&stop_signal))
        .expect("Error setting SIGQUIT handler");

    info!("creating prover, worker name: {}", worker_name);

    // Create client
    let api_url = env::var("PROVER_SERVER_URL").expect("PROVER_SERVER_URL is missing");
    let api_client = client::ApiClient::new(&api_url, &worker_name, Some(stop_signal.clone()));
    // Create prover
    let jubjub_params = AltJubjubBn256::new();
    let circuit_params = read_circuit_params(block_size_chunks);
    let heartbeat_interval = time::Duration::from_secs(PROVER_HEARTBEAT_INTERVAL);
    let worker = BabyProver::new(
        circuit_params,
        jubjub_params,
        block_size_chunks,
        api_client.clone(),
        heartbeat_interval,
        stop_signal,
    );

    let prover_id_arc = Arc::new(AtomicI32::new(ABSENT_PROVER_ID));

    // Handle termination requests.
    {
        let prover_id_arc = prover_id_arc.clone();
        let api_client = api_client.clone();
        thread::spawn(move || {
            let signals = Signals::new(&[
                signal_hook::SIGTERM,
                signal_hook::SIGINT,
                signal_hook::SIGQUIT,
            ])
            .expect("Signals::new() failed");
            for _ in signals.forever() {
                info!("Termination signal received.");
                let prover_id = prover_id_arc.load(Ordering::SeqCst);
                if prover_id != ABSENT_PROVER_ID {
                    match api_client.prover_stopped(prover_id) {
                        Ok(_) => {}
                        Err(e) => error!("failed to send prover stop request: {}", e),
                    }
                }

                std::process::exit(0);
            }
        });
    }

    // Register prover
    prover_id_arc.store(
        api_client
            .register_prover(block_size_chunks)
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
