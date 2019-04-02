extern crate server;
extern crate storage;
extern crate server_models;
extern crate prover;
extern crate ctrlc;
extern crate signal_hook;
extern crate tokio;

use std::sync::mpsc::{channel};

use prover::start_prover_handler;

use server::state_keeper::{PlasmaStateKeeper, start_state_keeper};
use server::api_server::start_api_server;
use server::committer;
use server::eth_sender;
use server::eth_watch::{EthWatch, start_eth_watch};

use storage::{ConnectionPool};
use server_models::StateKeeperRequest;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::runtime::Runtime;

fn main() {

    // handle ctrl+c
    let stop_signal = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::SIGTERM, Arc::clone(&stop_signal)).expect("Error setting SIGTERM handler");
    signal_hook::flag::register(signal_hook::SIGINT, Arc::clone(&stop_signal)).expect("Error setting SIGINT handler");
    signal_hook::flag::register(signal_hook::SIGQUIT, Arc::clone(&stop_signal)).expect("Error setting SIGQUIT handler");

    // create main tokio runtime
    //let rt = Runtime::new().unwrap();

    // create channel to accept deserialized requests for new transacitons
    let (tx_for_state, rx_for_state) = channel();
    let (tx_for_proof_requests, rx_for_proof_requests) = channel();
    let (tx_for_ops, rx_for_ops) = channel();

    let connection_pool = ConnectionPool::new();
    let state_keeper = PlasmaStateKeeper::new(connection_pool.clone());
    //let prover = BabyProver::create(connection_pool.clone()).unwrap();
    let eth_watch = EthWatch::new(0, 0, connection_pool.clone());

    // spawn threads for different processes
    // see https://docs.google.com/drawings/d/16UeYq7cuZnpkyMWGrgDAbmlaGviN2baY1w1y745Me70/edit?usp=sharing

    println!("starting actors");

    // Simple timer, pings every 15 seconds
    let tx_for_state_ticker = tx_for_state.clone();
    std::thread::Builder::new().name("timer".to_string()).spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(15));
            tx_for_state_ticker.send(StateKeeperRequest::TimerTick).expect("tx_for_state_ticker channel failed");
        }
    }).expect("thread creation failed");

    start_api_server(tx_for_state.clone(), connection_pool.clone());
    start_eth_watch(eth_watch, tx_for_state.clone());
    
    start_state_keeper(state_keeper, rx_for_state, tx_for_ops.clone());
    start_prover_handler(connection_pool.clone(), rx_for_proof_requests, tx_for_ops);

    let tx_for_eth = eth_sender::start_eth_sender(connection_pool.clone());

    std::thread::Builder::new().name("committer".to_string()).spawn(move || {
        committer::run_committer(rx_for_ops, tx_for_eth, tx_for_proof_requests, connection_pool.clone());
    }).expect("threade creation failed");

    while !stop_signal.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}