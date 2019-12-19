// Built-in deps
use std::sync::mpsc;
use std::sync::{atomic::AtomicBool, Arc};
use std::{env, thread, time};
// External deps
use bellman::groth16;
use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use log::{debug, error, info};
use pairing::bn256;
use signal_hook::iterator::Signals;
// Workspace deps
use models::node::config::{PROVER_GONE_TIMEOUT, PROVER_HEARTBEAT_INTERVAL};
use prover::client;
use prover::{start, BabyProver};

fn main() {
    env_logger::init();

    // handle ctrl+c
    let stop_signal = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::SIGTERM, Arc::clone(&stop_signal))
        .expect("Error setting SIGTERM handler");
    signal_hook::flag::register(signal_hook::SIGINT, Arc::clone(&stop_signal))
        .expect("Error setting SIGINT handler");
    signal_hook::flag::register(signal_hook::SIGQUIT, Arc::clone(&stop_signal))
        .expect("Error setting SIGQUIT handler");

    let worker_name = env::var("POD_NAME").unwrap_or_else(|_| "default".to_string());
    let key_dir = env::var("KEY_DIR").expect("KEY_DIR not set");
    info!("creating prover, worker name: {}", worker_name);

    // Create client
    let api_url = env::var("WITNESS_GENERATOR_API_URL").unwrap();
    let api_client = client::ApiClient::new(&api_url, &worker_name);
    // Create prover
    let jubjub_params = AltJubjubBn256::new();
    let circuit_params = read_from_key_dir(key_dir);
    let heartbeat_interval = time::Duration::from_secs(PROVER_HEARTBEAT_INTERVAL);
    let prover_timeout = time::Duration::from_secs(PROVER_GONE_TIMEOUT as u64);
    let worker = BabyProver::new(
        circuit_params,
        jubjub_params,
        api_client,
        heartbeat_interval,
        prover_timeout,
        stop_signal,
    );
    // Register prover
    let prover_id = client::ApiClient::new(&api_url, "")
        .register_prover()
        .expect("failed to register prover");
    // Start prover
    let (exit_err_tx, exit_err_rx) = mpsc::channel();
    thread::spawn(move || {
        start(worker, exit_err_tx);
    });

    // Handle termination requests.
    let prover_id_copy = prover_id;
    let api_url_copy = api_url.clone();
    thread::spawn(move || {
        let signals = Signals::new(&[
            signal_hook::SIGTERM,
            signal_hook::SIGINT,
            signal_hook::SIGQUIT,
        ])
        .expect("Signals::new() failed");
        for _ in signals.forever() {
            info!(
                "Termination signal received. Prover will finish the job and shut down gracefully"
            );
            client::ApiClient::new(&api_url_copy, "")
                .prover_stopped(prover_id_copy)
                .unwrap();
        }
    });

    // Handle prover exit errors.
    let err = exit_err_rx.recv();
    error!("prover exited with error: {:?}", err);
    client::ApiClient::new(&api_url, "")
        .prover_stopped(prover_id)
        .unwrap();
}

fn read_from_key_dir(key_dir: String) -> groth16::Parameters<bn256::Bn256> {
    let path = {
        let mut key_file_path = std::path::PathBuf::new();
        key_file_path.push(&key_dir);
        key_file_path.push(&format!("{}", models::params::block_size_chunks()));
        key_file_path.push(models::params::KEY_FILENAME);
        key_file_path
    };
    debug!("Reading key from {}", path.to_str().unwrap());
    let franklin_circuit_params = read_parameters(&path.to_str().unwrap());
    if franklin_circuit_params.is_err() {
        panic!("could not read circuit params")
    }
    franklin_circuit_params.unwrap()
}

fn read_parameters(file_name: &str) -> Result<groth16::Parameters<bn256::Bn256>, String> {
    use std::fs::File;
    use std::io::BufReader;

    let f_r = File::open(file_name);
    if f_r.is_err() {
        return Err(format!("could not open file {}", f_r.err().unwrap()));
    }
    let mut r = BufReader::new(f_r.unwrap());
    let circuit_params = groth16::Parameters::<bn256::Bn256>::read(&mut r, true);

    if circuit_params.is_err() {
        return Err(format!(
            "could not parse circuit params {}",
            circuit_params.err().unwrap()
        ));
    }

    Ok(circuit_params.unwrap())
}
