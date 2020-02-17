// Built-in deps
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::{env, thread, time};
// External deps
use crypto_exports::franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use crypto_exports::franklin_crypto::bellman::groth16;
use log::{debug, error, info};
use signal_hook::iterator::Signals;
// Workspace deps
use models::node::config::PROVER_HEARTBEAT_INTERVAL;
use models::node::Engine;
use models::prover_utils::read_circuit_proving_parameters;
use prover::client;
use prover::{start, BabyProver};

fn main() {
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

    let worker_name = env::var("POD_NAME").expect("POD_NAME is missing");
    let key_dir = env::var("KEY_DIR").expect("KEY_DIR not set");
    info!("creating prover, worker name: {}", worker_name);

    // Create client
    let api_url = env::var("PROVER_SERVER_URL").expect("PROVER_SERVER_URL is missing");
    let api_client = client::ApiClient::new(&api_url, &worker_name, Some(stop_signal.clone()));
    // Create prover
    let jubjub_params = AltJubjubBn256::new();
    let circuit_params = read_from_key_dir(key_dir);
    let heartbeat_interval = time::Duration::from_secs(PROVER_HEARTBEAT_INTERVAL);
    let worker = BabyProver::new(
        circuit_params,
        jubjub_params,
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
            .register_prover()
            .expect("failed to register prover"),
        Ordering::SeqCst,
    );

    // Start prover
    let (exit_err_tx, exit_err_rx) = mpsc::channel();
    thread::spawn(move || {
        start(worker, exit_err_tx);
    });

    // Handle prover exit errors.
    let err = exit_err_rx.recv();
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

fn read_from_key_dir(key_dir: String) -> groth16::Parameters<Engine> {
    let path = {
        let mut key_file_path = std::path::PathBuf::new();
        key_file_path.push(&key_dir);
        key_file_path.push(&format!("{}", models::params::block_size_chunks()));
        key_file_path.push(&format!("{}", models::params::account_tree_depth()));
        key_file_path.push(models::params::KEY_FILENAME);
        key_file_path
    };
    debug!("Reading key from {}", path.to_string_lossy());
    read_circuit_proving_parameters(&path).expect("Failed to read circuit parameters")
}
