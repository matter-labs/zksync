#[macro_use]
extern crate log;

pub mod tree_state;
pub mod data_restore_driver;
pub mod events;
pub mod events_state;
pub mod rollup_ops;
pub mod genesis_state;
pub mod helpers;
pub mod storage_interactor;

use crate::data_restore_driver::DataRestoreDriver;
use storage::ConnectionPool;
use web3::types::{H160, H256};
use web3::Transport;

pub fn create_data_restore_driver_empty(
    connection_pool: ConnectionPool,
    web3_url: String,
    contract_eth_addr: H160,
    eth_blocks_step: u64,
    end_eth_blocks_offset: u64
) -> Result<DataRestoreDriver<web3::transports::Http>, failure::Error> {
    let (_eloop, transport) = web3::transports::Http::new(&web3_url).unwrap();
    let web3 = web3::Web3::new(transport);
    DataRestoreDriver::new_empty(
        connection_pool,
        web3,
        contract_eth_addr,
        eth_blocks_step,
        end_eth_blocks_offset
    )
}

/// Creates data restore driver state
///
/// # Arguments
///
/// * `connection_pool` - Database connection pool
///
pub fn create_data_restore_driver_with_genesis(
    connection_pool: ConnectionPool,
    web3_url: String,
    contract_eth_addr: H160,
    contract_genesis_tx_hash: H256,
    eth_blocks_step: u64,
    end_eth_blocks_offset: u64
) -> Result<DataRestoreDriver<web3::transports::Http>, failure::Error> {
    let (_eloop, transport) = web3::transports::Http::new(&web3_url).unwrap();
    let web3 = web3::Web3::new(transport);
    DataRestoreDriver::new_from_genesis(
        connection_pool,
        web3,
        contract_eth_addr,
        contract_genesis_tx_hash,
        eth_blocks_step,
        end_eth_blocks_offset
    )
}

/// Loads states from storage and start update
pub fn load_state_from_storage<T: Transport>(driver: &mut DataRestoreDriver<T>) {
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
pub fn update_state<T: Transport>(driver: &mut DataRestoreDriver<T>) {
    driver.run_state_update();
}

pub fn stop_state_update<T: Transport>(driver: &mut DataRestoreDriver<T>) {
    driver.stop_state_update();
}
