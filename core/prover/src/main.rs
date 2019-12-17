// Built-in deps
use std::str::FromStr;
use std::sync::{atomic::AtomicBool, Arc};
use std::{env, thread, time};
// External deps
use bellman::groth16;
use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use log::{debug, error, info};
use pairing::bn256;
use signal_hook::iterator::Signals;
use tokio::runtime::current_thread::Runtime;
use tokio::sync::oneshot;
// Workspace deps
use prover::witness_generator::client;
use prover::{start, Worker};

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
    let heartbeat_interval =
        env::var("HEARTBEAT_INTERVAL_MILLISEC").unwrap_or_else(|_| "1000".to_owned());
    let heartbeat_interval = u64::from_str(&heartbeat_interval).unwrap();
    let heartbeat_interval = time::Duration::from_millis(heartbeat_interval);
    let worker = Worker::new(
        circuit_params,
        jubjub_params,
        api_client,
        heartbeat_interval,
        stop_signal,
    );
    // Register prover
    let api_client = client::ApiClient::new(&api_url, "");
    let _prover_id = api_client
        .register_prover()
        .expect("failed to register prover");
    // Start prover
    thread::spawn(move || {
        start(worker);
    });

    let signals = Signals::new(&[
        signal_hook::SIGTERM,
        signal_hook::SIGINT,
        signal_hook::SIGQUIT,
    ])
    .expect("Signals::new() failed");
    for _ in signals.forever() {
        info!("Termination signal received. Prover will finish the job and shut down gracefully");
        // TODO: on terminate signal, send prover stop request
    }
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
