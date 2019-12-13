#[macro_use]
extern crate log;

pub mod accounts_state;
pub mod data_restore_driver;
pub mod events;
pub mod events_state;
pub mod franklin_ops;
pub mod genesis_state;
pub mod helpers;
pub mod storage_interactor;

use crate::data_restore_driver::DataRestoreDriver;
use storage::ConnectionPool;
use web3::types::{H160, H256};
use web3::Transport;

/// Creates data restore driver state
///
/// # Arguments
///
/// * `connection_pool` - Database connection pool
///
fn create_data_restore_driver(
    connection_pool: ConnectionPool,
    web3_url: String,
    contract_eth_addr: H160,
    contract_genesis_tx_hash: H256,
    eth_blocks_step: u64,
    end_eth_blocks_offset: u64
) -> Result<DataRestoreDriver<web3::transports::Http>, failure::Error> {
    let (_eloop, transport) = web3::transports::Http::new(&web3_url).unwrap();
    let web3 = web3::Web3::new(transport);
    return DataRestoreDriver::new(connection_pool, web3, contract_eth_addr, contract_genesis_tx_hash, eth_blocks_step, end_eth_blocks_offset)
}

/// Loads states from storage and start update
fn load_state<T: Transport>(driver: &mut DataRestoreDriver<T>) {
    driver
        .load_state_from_storage()
        .expect("Cant load state");
}

/// Runs states updates
///
/// # Arguments
///
/// * `driver` - DataRestore Driver config
///
fn run_state_updates<T: Transport>(driver: &mut DataRestoreDriver<T>) {
    driver.run_state_updates();
}

fn stop_state_updates<T: Transport>(driver: &mut DataRestoreDriver<T>) {
    // driver.stop_state_updates().expect("Cant stop updates");
}
