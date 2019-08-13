//use tokio::runtime::Runtime;
#[macro_use]
extern crate log;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use server::api_server::start_api_server;
use server::committer::start_committer;
use server::eth_sender;
use server::eth_watch::{start_eth_watch, EthWatch};
use server::state_keeper::{start_state_keeper, PlasmaStateKeeper};

use models::{node::config, StateKeeperRequest};
use storage::ConnectionPool;

fn main() {
    env_logger::init();

    debug!("starting server");

    // handle ctrl+c
    let stop_signal = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::SIGTERM, Arc::clone(&stop_signal))
        .expect("Error setting SIGTERM handler");
    signal_hook::flag::register(signal_hook::SIGINT, Arc::clone(&stop_signal))
        .expect("Error setting SIGINT handler");
    signal_hook::flag::register(signal_hook::SIGQUIT, Arc::clone(&stop_signal))
        .expect("Error setting SIGQUIT handler");

    // create main tokio runtime
    //let rt = Runtime::new().unwrap();

    let connection_pool = ConnectionPool::new();
    let eth_watch = EthWatch::new();
    let state_keeper =
        PlasmaStateKeeper::new(connection_pool.clone(), eth_watch.get_shared_eth_state());

    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for committer");
    let contract_addr = storage
        .load_config()
        .expect("can not load server_config")
        .contract_addr
        .expect("contract_addr empty in server_config");
    if contract_addr != config::RUNTIME_CONFIG.contract_addr {
        panic!(
            "Contract addresses mismatch! From DB = {}, from env = {}",
            contract_addr,
            config::RUNTIME_CONFIG.contract_addr
        );
    }
    drop(storage);

    // spawn threads for different processes
    // see https://docs.google.com/drawings/d/16UeYq7cuZnpkyMWGrgDAbmlaGviN2baY1w1y745Me70/edit?usp=sharing

    info!("starting actors");

    let (tx_for_state, rx_for_state) = channel();
    start_api_server(tx_for_state.clone(), connection_pool.clone());
    start_eth_watch(eth_watch);
    let (tx_for_ops, rx_for_ops) = channel();
    start_state_keeper(state_keeper, rx_for_state, tx_for_ops.clone());
    let tx_for_eth = eth_sender::start_eth_sender(connection_pool.clone());
    start_committer(rx_for_ops, tx_for_eth, connection_pool.clone());

    // start_prover(connection_pool.clone(), "worker 1");
    // start_prover(connection_pool.clone(), "worker 2");
    // start_prover(connection_pool.clone(), "worker 3");

    // Simple timer, pings every 100 ms
    thread::Builder::new()
        .name("timer".to_string())
        .spawn(move || loop {
            tx_for_state
                .send(StateKeeperRequest::TimerTick)
                .expect("tx_for_state channel failed");
            thread::sleep(Duration::from_millis(100));
        })
        .expect("thread creation failed");

    while !stop_signal.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_secs(1));
    }

    info!("terminate signal received");
}
