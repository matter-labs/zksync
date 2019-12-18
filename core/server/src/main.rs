//use tokio::runtime::Runtime;
#[macro_use]
extern crate log;
// Built-in uses
use std::thread;
use std::time::Duration;
// External uses
use clap::{App, Arg};
use futures::{channel::mpsc, executor::block_on, SinkExt, StreamExt};
use tokio::runtime::Runtime;
// Workspace uses
use server::api_server::start_api_server;
use server::block_proposer::run_block_proposer_task;
use server::committer::run_committer;
use server::eth_watch::start_eth_watch;
use server::mempool::run_mempool_task;
use server::state_keeper::{start_state_keeper, PlasmaStateKeeper};
use server::{eth_sender, ConfigurationOptions, ThreadPanicNotify};
use std::cell::RefCell;
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

    let mut main_runtime = Runtime::new().expect("main runtime start");

    // handle ctrl+c
    let (stop_signal_sender, mut stop_signal_receiver) = mpsc::channel(256);
    {
        let stop_signal_sender = RefCell::new(stop_signal_sender.clone());
        ctrlc::set_handler(move || {
            let mut sender = stop_signal_sender.borrow_mut();
            block_on(sender.send(true)).expect("crtlc signal send");
        })
        .expect("Error setting Ctrl-C handler");
    }

    let shared_eth_state = start_eth_watch(
        connection_pool.clone(),
        stop_signal_sender.clone(),
        config_opts.clone(),
    );

    let (proposed_blocks_sender, proposed_blocks_receiver) = mpsc::channel(256);
    let (state_keeper_req_sender, state_keeper_req_receiver) = mpsc::channel(256);
    let state_keeper = PlasmaStateKeeper::new(
        connection_pool.clone(),
        config_opts.operator_franklin_addr.clone(),
        state_keeper_req_receiver,
        proposed_blocks_sender,
    );
    start_state_keeper(state_keeper, &main_runtime);

    let (eth_send_request_sender, eth_send_request_receiver) = mpsc::channel(256);
    let (zksync_commit_notify_sender, zksync_commit_notify_receiver) = mpsc::channel(256);
    eth_sender::start_eth_sender(
        connection_pool.clone(),
        stop_signal_sender.clone(),
        zksync_commit_notify_sender.clone(), // eth sender sends only verify blocks notifications
        eth_send_request_receiver,
        config_opts.clone(),
    );

    run_committer(
        proposed_blocks_receiver,
        eth_send_request_sender,
        zksync_commit_notify_sender, // commiter sends only commit block notifications
        connection_pool.clone(),
        &main_runtime,
    );
    start_api_server(
        zksync_commit_notify_receiver,
        connection_pool.clone(),
        stop_signal_sender.clone(),
        config_opts.clone(),
    );

    let (mempool_request_sender, mempool_request_receiver) = mpsc::channel(256);
    run_mempool_task(
        shared_eth_state,
        connection_pool.clone(),
        mempool_request_receiver,
        state_keeper_req_sender.clone(),
        &main_runtime,
    );
    run_block_proposer_task(
        mempool_request_sender,
        state_keeper_req_sender,
        &main_runtime,
    );

    main_runtime.block_on(async move { stop_signal_receiver.next().await });
}
