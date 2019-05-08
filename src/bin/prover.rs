extern crate storage;
extern crate prover;
extern crate signal_hook;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::env;

use prover::start_prover;

fn main() {

    // handle ctrl+c
    let stop_signal = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::SIGTERM, Arc::clone(&stop_signal)).expect("Error setting SIGTERM handler");
    signal_hook::flag::register(signal_hook::SIGINT, Arc::clone(&stop_signal)).expect("Error setting SIGINT handler");
    signal_hook::flag::register(signal_hook::SIGQUIT, Arc::clone(&stop_signal)).expect("Error setting SIGQUIT handler");

    let args: Vec<String> = env::args().collect();
    start_prover(args.get(1).unwrap_or(&"default".to_string()).clone());

    while !stop_signal.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_secs(1));
    }

    println!("terminate signal received");
}