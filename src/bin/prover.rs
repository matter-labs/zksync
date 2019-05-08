extern crate storage;
extern crate prover;
extern crate signal_hook;
extern crate tokio;
extern crate futures;

use std::sync::{Arc, atomic::{AtomicBool}};
use std::env;
use prover::run_prover;
use signal_hook::iterator::Signals;
use futures::Stream;
use tokio::runtime::current_thread::Runtime;
use tokio::sync::oneshot;
use std::thread;

fn main() {

    // handle ctrl+c
    let stop_signal = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::SIGTERM, Arc::clone(&stop_signal)).expect("Error setting SIGTERM handler");
    signal_hook::flag::register(signal_hook::SIGINT, Arc::clone(&stop_signal)).expect("Error setting SIGINT handler");
    signal_hook::flag::register(signal_hook::SIGQUIT, Arc::clone(&stop_signal)).expect("Error setting SIGQUIT handler");

    let mut rt = Runtime::new().unwrap();
    let handle = rt.handle();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    // Run tokio timeline in a new thread
    thread::spawn(move || {
        let name = env::var("POD_NAME").unwrap_or("default".to_string());
        run_prover(shutdown_tx, &handle, stop_signal, name);        
    });

    // this just prints info message
    rt.spawn(
        Signals::new(&[signal_hook::SIGTERM, signal_hook::SIGINT, signal_hook::SIGQUIT])
        .unwrap()
        .into_async()
        .unwrap()
        .map_err(|_|())
        .for_each(|_| {println!("Termination signal received. Prover will finish the job and shut down gracefully"); Ok(())})
    );
    rt.block_on(shutdown_rx).unwrap();

}