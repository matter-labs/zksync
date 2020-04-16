//use tokio::runtime::Runtime;
#[macro_use]
extern crate log;
// External uses
use clap::{App, Arg};
// Workspace uses
use futures::{channel::mpsc, executor::block_on, SinkExt, StreamExt};
use models::config_options::ConfigurationOptions;
use models::node::config::{
    OBSERVER_MODE_PULL_INTERVAL, PROVER_GONE_TIMEOUT, PROVER_PREPARE_DATA_INTERVAL,
};
use server::api_server::start_api_server;
use server::block_proposer::run_block_proposer_task;
use server::committer::run_committer;
use server::eth_watch::start_eth_watch;
use server::mempool::run_mempool_task;
use server::prover_server::start_prover_server;
use server::state_keeper::{start_state_keeper, PlasmaStateKeeper};
use server::{eth_sender, leader_election, observer_mode};
use std::cell::RefCell;
use std::time::Duration;
use storage::ConnectionPool;
use tokio::runtime::Runtime;
use web3::types::H160;

fn main() {
    env_logger::init();

    let config_opts = ConfigurationOptions::from_env();

    let cli = App::new("ZKSync operator node")
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
            connection_pool,
            &config_opts.operator_franklin_addr,
        );
        return;
    }

    debug!("starting server");

    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for committer");
    let contract_addr: H160 = storage
        .config_schema()
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

    // Start observing the state and try to become leader.
    let (stop_observer_mode_tx, stop_observer_mode_rx) = std::sync::mpsc::channel();
    let (observed_state_tx, observed_state_rx) = std::sync::mpsc::channel();
    let conn_pool_clone = connection_pool.clone();
    let jh = std::thread::Builder::new()
        .name("Observer mode".to_owned())
        .spawn(move || {
            let state = block_on(observer_mode::run(
                conn_pool_clone.clone(),
                OBSERVER_MODE_PULL_INTERVAL,
                stop_observer_mode_rx,
            ));
            observed_state_tx.send(state).expect("unexpected failure");
        })
        .expect("failed to start observer mode");
    block_on(leader_election::keep_voting_to_be_leader(
        config_opts.replica_name.clone(),
        connection_pool.clone(),
    ))
    .expect("voting for leader fail");
    stop_observer_mode_tx.send(()).expect("unexpected failure");
    let observer_mode_final_state = observed_state_rx.recv().expect("unexpected failure");
    jh.join().unwrap();

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

    let (eth_watch_req_sender, eth_watch_req_receiver) = mpsc::channel(256);
    start_eth_watch(
        connection_pool.clone(),
        config_opts.clone(),
        eth_watch_req_sender.clone(),
        eth_watch_req_receiver,
        &main_runtime,
    );

    let (proposed_blocks_sender, proposed_blocks_receiver) = mpsc::channel(256);
    let (state_keeper_req_sender, state_keeper_req_receiver) = mpsc::channel(256);
    let (executed_tx_notify_sender, executed_tx_notify_receiver) = mpsc::channel(256);
    let (mempool_request_sender, mempool_request_receiver) = mpsc::channel(256);
    let state_keeper = PlasmaStateKeeper::new(
        observer_mode_final_state.state_keeper_init,
        config_opts.operator_franklin_addr,
        state_keeper_req_receiver,
        proposed_blocks_sender,
        executed_tx_notify_sender,
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
        mempool_request_sender.clone(),
        connection_pool.clone(),
        &main_runtime,
    );
    start_api_server(
        zksync_commit_notify_receiver,
        connection_pool.clone(),
        stop_signal_sender.clone(),
        mempool_request_sender.clone(),
        executed_tx_notify_receiver,
        state_keeper_req_sender.clone(),
        eth_watch_req_sender.clone(),
        config_opts.clone(),
    );
    start_prover_server(
        connection_pool.clone(),
        config_opts.prover_server_address,
        Duration::from_secs(PROVER_GONE_TIMEOUT as u64),
        Duration::from_secs(PROVER_PREPARE_DATA_INTERVAL),
        stop_signal_sender,
        observer_mode_final_state.circuit_acc_tree,
        observer_mode_final_state.circuit_tree_block,
    );

    run_mempool_task(
        connection_pool,
        mempool_request_receiver,
        eth_watch_req_sender,
        &main_runtime,
    );
    run_block_proposer_task(
        mempool_request_sender,
        state_keeper_req_sender,
        &main_runtime,
    );

    main_runtime.block_on(async move { stop_signal_receiver.next().await });
}
