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

fn main() {

    let mut rt = tokio::runtime::Runtime::new().unwrap();

    // handle ctrl+c
    let stop_signal = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::SIGTERM, Arc::clone(&stop_signal)).expect("Error setting SIGTERM handler");
    signal_hook::flag::register(signal_hook::SIGINT, Arc::clone(&stop_signal)).expect("Error setting SIGINT handler");
    signal_hook::flag::register(signal_hook::SIGQUIT, Arc::clone(&stop_signal)).expect("Error setting SIGQUIT handler");

    rt.spawn(
        Signals::new(&[signal_hook::SIGTERM, signal_hook::SIGINT, signal_hook::SIGQUIT])
        .unwrap()
        .into_async()
        .unwrap()
        .map_err(|_|())
        .for_each(|_| {println!("termination signal received"); Ok(())})
    );

    run_prover(stop_signal, env::var("POD_NAME").unwrap_or("default".to_string()).clone());    

    rt.shutdown_now();
    println!("prover terminated");
}