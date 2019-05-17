extern crate storage;
extern crate prover;
extern crate signal_hook;
extern crate tokio;
extern crate futures;

use std::sync::{Arc, atomic::{AtomicBool}};
use std::env;
use prover::BabyProver;
use signal_hook::iterator::Signals;
use tokio::runtime::current_thread::Runtime;
use tokio::sync::oneshot;
use std::thread;
use storage::StorageProcessor;

fn main() {

    // handle ctrl+c
    let stop_signal = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::SIGTERM, Arc::clone(&stop_signal)).expect("Error setting SIGTERM handler");
    signal_hook::flag::register(signal_hook::SIGINT, Arc::clone(&stop_signal)).expect("Error setting SIGINT handler");
    signal_hook::flag::register(signal_hook::SIGQUIT, Arc::clone(&stop_signal)).expect("Error setting SIGQUIT handler");

    let worker = env::var("POD_NAME").unwrap_or("default".to_string());
    println!("creating prover, worker: {}", worker);
    let mut prover = BabyProver::create(worker).unwrap();
    let prover_id = prover.prover_id;

    let mut rt = Runtime::new().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    prover.start_timer_interval(&rt.handle());

    // Run tokio timeline in a new thread
    thread::spawn(move || {
        prover.run(shutdown_tx, stop_signal);        
    });

    let signals = Signals::new(&[signal_hook::SIGTERM, signal_hook::SIGINT, signal_hook::SIGQUIT]).expect("Signals::new() failed");
    thread::spawn(move || {
        for _ in signals.forever() {
            println!("Termination signal received. Prover will finish the job and shut down gracefully"); 
            let storage = StorageProcessor::establish_connection().expect("db connection failed for prover");
            storage.record_prover_stop(prover_id).expect("db failed");
        }
    });

    rt.block_on(shutdown_rx).unwrap();

}