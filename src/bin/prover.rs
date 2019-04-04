extern crate storage;
extern crate prover;
extern crate signal_hook;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::env;

use prover::start_prover;
use storage::ConnectionPool;

fn main() {

    // handle ctrl+c
    let stop_signal = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::SIGTERM, Arc::clone(&stop_signal)).expect("Error setting SIGTERM handler");
    signal_hook::flag::register(signal_hook::SIGINT, Arc::clone(&stop_signal)).expect("Error setting SIGINT handler");
    signal_hook::flag::register(signal_hook::SIGQUIT, Arc::clone(&stop_signal)).expect("Error setting SIGQUIT handler");

    let args: Vec<String> = env::args().collect();
    // if args.len() < 2 {
    //     println!("Usage: prover <worker_name>");
    //     return;
    // }

    let connection_pool = ConnectionPool::new();
    start_prover(connection_pool.clone(), args.get(1).unwrap_or(&"default".to_string()).clone());

    while !stop_signal.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_secs(1));
    }
}