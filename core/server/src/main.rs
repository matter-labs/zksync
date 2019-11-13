//use tokio::runtime::Runtime;
#[macro_use]
extern crate log;

use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use server::api_server::start_api_server;
use server::committer::start_committer;
use server::eth_watch::start_eth_watch;
use server::state_keeper::{start_state_keeper, PlasmaStateKeeper};
use server::{eth_sender, ThreadPanicNotify};

use models::{node::config, StateKeeperRequest};
use storage::ConnectionPool;

use clap::{App, Arg};

fn main() {
    env_logger::init();

    let cmd_line = App::new("Franklin operator node")
        .author("Matter labs")
        .arg(
            Arg::with_name("genesis")
                .long("genesis")
                .help("Generate genesis block for the first contract deployment"),
        )
        .get_matches();

    let connection_pool = ConnectionPool::new();

    if cmd_line.is_present("genesis") {
        info!("Generating genesis block.");
        PlasmaStateKeeper::create_genesis_block(connection_pool.clone());
        return;
    }

    debug!("starting server");

    // handle ctrl+c
    let (stop_signal_sender, stop_signal_receiver) = channel();
    {
        let stop_signal_sender = stop_signal_sender.clone();
        ctrlc::set_handler(move || {
            stop_signal_sender.send(true).expect("crtlc signal send");
        })
        .expect("Error setting Ctrl-C handler");
    }

    // create main tokio runtime
    //let rt = Runtime::new().unwrap();

    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for committer");
    let server_config = storage.load_config().expect("can not load server_config");
    let contract_addr = server_config.contract_addr.expect("server config is empty");
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
    let shared_eth_state = start_eth_watch(connection_pool.clone(), stop_signal_sender.clone());
    let (tx_for_ops, rx_for_ops) = channel();
    let state_keeper = PlasmaStateKeeper::new(connection_pool.clone(), shared_eth_state);
    start_state_keeper(
        state_keeper,
        rx_for_state,
        tx_for_ops.clone(),
        stop_signal_sender.clone(),
    );
    let (tx_for_eth, ops_notify_receiver) =
        eth_sender::start_eth_sender(connection_pool.clone(), stop_signal_sender.clone());
    start_committer(
        rx_for_ops,
        tx_for_eth,
        connection_pool.clone(),
        stop_signal_sender.clone(),
    );
    start_api_server(
        ops_notify_receiver,
        connection_pool.clone(),
        stop_signal_sender.clone(),
    );

    // Simple timer, pings every 100 ms
    thread::Builder::new()
        .name("timer".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(stop_signal_sender);
            loop {
                tx_for_state
                    .send(StateKeeperRequest::TimerTick)
                    .expect("tx_for_state channel failed");
                thread::sleep(Duration::from_millis(100));
            }
        })
        .expect("thread creation failed");

    stop_signal_receiver.recv().expect("stop signal receive");

    info!("terminate signal received");
}
