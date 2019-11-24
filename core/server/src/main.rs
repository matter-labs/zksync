//use tokio::runtime::Runtime;
#[macro_use]
extern crate log;
// Built-in uses
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;
// External uses
use clap::{App, Arg};
// Workspace uses
use models::StateKeeperRequest;
use server::api_server::start_api_server;
use server::committer::start_committer;
use server::eth_watch::start_eth_watch;
use server::state_keeper::{start_state_keeper, PlasmaStateKeeper};
use server::{eth_sender, ConfigurationOptions, ThreadPanicNotify};
use storage::ConnectionPool;
use web3::types::H160;

fn main() {
    env_logger::init();

    let config_opts = ConfigurationOptions::from_env();

    let cli = App::new("Franklin operator node")
        .author("Matter Labs")
        .arg(
            Arg::with_name("genesis")
                .long("genesis")
                .help("Generate genesis block for the first contract deployment"),
        )
        .get_matches();

    let connection_pool = ConnectionPool::new();

    if cli.is_present("genesis") {
        info!("Generating genesis block.");
        PlasmaStateKeeper::create_genesis_block(
            connection_pool.clone(),
            &config_opts.operator_franklin_addr,
        );
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

    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for committer");
    let contract_addr: H160 = storage
        .load_config()
        .expect("can not load server_config")
        .contract_addr
        .expect("contract_addr empty in server_config")[2..]
        .parse()
        .expect("contract_addr in db wrong");
    if contract_addr != config_opts.contract_eth_addr {
        panic!(
            "Contract addresses mismatch! From DB = {}, from env = {}",
            contract_addr, config_opts.contract_eth_addr
        );
    }
    drop(storage);

    // spawn threads for different processes
    // see https://docs.google.com/drawings/d/16UeYq7cuZnpkyMWGrgDAbmlaGviN2baY1w1y745Me70/edit?usp=sharing

    info!("starting actors");

    let (tx_for_state, rx_for_state) = channel();
    start_api_server(
        tx_for_state.clone(),
        connection_pool.clone(),
        stop_signal_sender.clone(),
        config_opts.clone(),
    );
    let shared_eth_state = start_eth_watch(
        connection_pool.clone(),
        stop_signal_sender.clone(),
        config_opts.clone(),
    );
    let (tx_for_ops, rx_for_ops) = channel();
    let state_keeper = PlasmaStateKeeper::new(
        connection_pool.clone(),
        shared_eth_state,
        config_opts.operator_franklin_addr.clone(),
        config_opts.tx_batch_size,
    );
    start_state_keeper(
        state_keeper,
        rx_for_state,
        tx_for_ops.clone(),
        stop_signal_sender.clone(),
    );
    let tx_for_eth = eth_sender::start_eth_sender(
        connection_pool.clone(),
        stop_signal_sender.clone(),
        config_opts.clone(),
    );
    start_committer(
        rx_for_ops,
        tx_for_eth,
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
