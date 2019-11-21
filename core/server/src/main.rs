//use tokio::runtime::Runtime;
#[macro_use]
extern crate log;
// Built-in uses
use std::env;
use std::str::FromStr;
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
use server::{eth_sender, ThreadPanicNotify};
use storage::ConnectionPool;

struct ConfigurationOptions {
    api_server_addr: String,
    api_server_port: String,
    contract_eth_addr: String,
    web3_url: String,
    governance_eth_addr: String,
    priority_queue_eth_addr: String,
    operator_franklin_addr: String,
    operator_eth_addr: String,
    operator_private_key: String,
    chain_id: u8,
    gas_price_factor: usize,
    tx_batch_size: usize,
}

impl ConfigurationOptions {
    pub fn from_env() -> ConfigurationOptions {
        let chain_id = env::var("CHAIN_ID").unwrap_or_else(|_| "4".to_string());
        let chain_id = u8::from_str(&chain_id).expect("CHAIN_ID invalid value");

        let gas_price_factor = env::var("GAS_PRICE_FACTOR").unwrap_or_else(|_| "1".to_string());
        let gas_price_factor =
            usize::from_str(&gas_price_factor).expect("gas price factor invalid");

        let tx_batch_size = env::var("TX_BATCH_SIZE").expect("TX_BATCH_SIZE env var missing");
        let tx_batch_size = usize::from_str(&tx_batch_size).expect("TX_BATCH_SIZE invalid value");
        ConfigurationOptions {
            api_server_addr: env::var("BIND_TO").unwrap_or_else(|_| "127.0.0.1".to_string()),
            api_server_port: env::var("PORT").unwrap_or_else(|_| "8080".to_string()),
            contract_eth_addr: env::var("CONTRACT_ADDR").expect("CONTRACT_ADDR env var missing"),
            web3_url: env::var("WEB3_URL").expect("WEB3_URL env var missing"),
            governance_eth_addr: env::var("GOVERNANCE_ADDR")
                .expect("GOVERNANCE_ADDR env var missing"),
            priority_queue_eth_addr: env::var("PRIORITY_QUEUE_ADDR")
                .expect("PRIORITY_QUEUE_ADDR env var missing"),
            operator_franklin_addr: env::var("OPERATOR_FRANKLIN_ADDRESS")
                .expect("OPERATOR_FRANKLIN_ADDRESS env var missing"),
            operator_eth_addr: env::var("OPERATOR_ETH_ADDRESS")
                .expect("OPERATOR_ETH_ADDRESS env var missing"),
            operator_private_key: env::var("OPERATOR_PRIVATE_KEY")
                .expect("OPERATOR_ETH_ADDRESS env var missing"),
            chain_id,
            gas_price_factor,
            tx_batch_size,
        }
    }
}

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
            config_opts.operator_franklin_addr.clone(),
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
    let contract_addr = storage
        .load_config()
        .expect("can not load server_config")
        .contract_addr
        .expect("contract_addr empty in server_config");
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
        config_opts.api_server_addr.clone(),
        config_opts.api_server_port.clone(),
        config_opts.contract_eth_addr.clone(),
    );
    let shared_eth_state = start_eth_watch(
        connection_pool.clone(),
        stop_signal_sender.clone(),
        config_opts.web3_url.clone(),
        config_opts.governance_eth_addr.clone(),
        config_opts.priority_queue_eth_addr.clone(),
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
        eth_sender::StartEthSenderOptions {
            web3_url: config_opts.web3_url.clone(),
            operator_eth_addr: config_opts.operator_eth_addr.clone(),
            operator_pk: config_opts.operator_private_key.clone(),
            contract_eth_addr: config_opts.contract_eth_addr.clone(),
            chain_id: config_opts.chain_id,
            gas_price_factor: config_opts.gas_price_factor,
        },
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
