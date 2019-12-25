#[macro_use]
extern crate log;

pub mod data_restore_driver;
pub mod events;
pub mod events_state;
pub mod genesis_state;
pub mod helpers;
pub mod rollup_ops;
pub mod storage_interactor;
pub mod tree_state;

use crate::data_restore_driver::DataRestoreDriver;
use clap::{App, Arg};
use server::ConfigurationOptions;
use storage::ConnectionPool;
use web3::transports::Http;
use web3::Transport;

const ETH_BLOCKS_STEP: u64 = 1000;
const END_ETH_BLOCKS_OFFSET: u64 = 40;

fn main() {
    info!("Building Franklin accounts state");
    env_logger::init();
    let connection_pool = ConnectionPool::new();
    let config_opts = ConfigurationOptions::from_env();

    let cli = App::new("Data restore driver")
        .author("Matter Labs")
        .arg(
            Arg::with_name("genesis")
                .long("genesis")
                .help("Restores data with provided genesis (zero) block"),
        )
        .arg(
            Arg::with_name("continue")
                .long("continue")
                .help("Continues data restoreing"),
        )
        .get_matches();

    let (_event_loop, transport) =
        Http::new(&config_opts.web3_url).expect("failed to start web3 transport");
    let governance_addr = config_opts.governance_eth_addr.clone();
    let governance_genesis_tx_hash = config_opts.governance_genesis_tx_hash.clone();
    let contract_addr = config_opts.contract_eth_addr.clone();
    let contract_genesis_tx_hash = config_opts.contract_genesis_tx_hash.clone();

    // If genesis is argument is present - there will be fetching contracts creation transactions to get first eth block and genesis acc address
    let mut driver = if cli.is_present("genesis") {
        DataRestoreDriver::new_with_genesis_acc(
            connection_pool,
            transport,
            governance_addr,
            governance_genesis_tx_hash,
            contract_addr,
            contract_genesis_tx_hash,
            ETH_BLOCKS_STEP,
            END_ETH_BLOCKS_OFFSET,
        )
    } else {
        DataRestoreDriver::new_empty(
            connection_pool,
            transport,
            governance_addr,
            contract_addr,
            ETH_BLOCKS_STEP,
            END_ETH_BLOCKS_OFFSET,
        )
    }
    .expect("Cant load state");

    if cli.is_present("continue") {
        load_state_from_storage(&mut driver)
    }

    update_state(&mut driver);
}

/// Loads states for driver from storage
///
/// # Arguments
///
/// * `driver` - Data restore driver instance
///
pub fn load_state_from_storage<T: Transport>(driver: &mut DataRestoreDriver<T>) {
    driver.load_state_from_storage().expect("Cant load state");
}

/// Runs states updates for driver
///
/// # Arguments
///
/// * `driver` - Data restore driver instance
///
pub fn update_state<T: Transport>(driver: &mut DataRestoreDriver<T>) {
    driver.run_state_update().expect("Cant update state");
}

/// Stopss states updates for driver
///
/// # Arguments
///
/// * `driver` - Data restore driver instance
///
pub fn stop_state_update<T: Transport>(driver: &mut DataRestoreDriver<T>) {
    driver.stop_state_update();
}
