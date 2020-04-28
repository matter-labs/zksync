#[macro_use]
extern crate log;

pub mod contract_functions;
pub mod data_restore_driver;
pub mod eth_tx_helpers;
pub mod events;
pub mod events_state;
pub mod rollup_ops;
pub mod storage_interactor;
pub mod tree_state;

use crate::data_restore_driver::DataRestoreDriver;
use clap::{App, Arg};
use models::config_options::ConfigurationOptions;
use storage::ConnectionPool;
use web3::transports::Http;

const ETH_BLOCKS_STEP: u64 = 1;
const END_ETH_BLOCKS_OFFSET: u64 = 40;

fn main() {
    info!("Restoring zkSync state from the contract");
    env_logger::init();
    let connection_pool = ConnectionPool::new(Some(1));
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
                .help("Continues data restoring"),
        )
        .get_matches();

    let (_event_loop, transport) =
        Http::new(&config_opts.web3_url).expect("failed to start web3 transport");
    let governance_addr = config_opts.governance_eth_addr;
    let governance_genesis_tx_hash = config_opts.governance_genesis_tx_hash;
    let contract_addr = config_opts.contract_eth_addr;
    let contract_genesis_tx_hash = config_opts.contract_genesis_tx_hash;
    let available_block_chunk_sizes = config_opts.available_block_chunk_sizes;

    let mut driver = DataRestoreDriver::new(
        connection_pool,
        transport,
        governance_addr,
        contract_addr,
        ETH_BLOCKS_STEP,
        END_ETH_BLOCKS_OFFSET,
        available_block_chunk_sizes,
    );

    // If genesis is argument is present - there will be fetching contracts creation transactions to get first eth block and genesis acc address
    if cli.is_present("genesis") {
        driver.set_genesis_state(governance_genesis_tx_hash, contract_genesis_tx_hash);
    }

    if cli.is_present("continue") {
        driver.load_state_from_storage();
    }

    driver.run_state_update();
}
