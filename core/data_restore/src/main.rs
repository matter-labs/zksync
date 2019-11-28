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

/// Step of the considered blocks ethereum block
const ETH_BLOCKS_DELTA: u64 = 250;
/// Delta between last ethereum block and last watched ethereum block to prevent restart in case of reorder
const END_ETH_BLOCKS_DELTA: u64 = 25;

fn main() {
    env_logger::init();
    info!("Building Franklin accounts state");
    load_state();
}

/// Creates data restore driver state
///
/// # Arguments
///
/// * `connection_pool` - Database connection pool
///
fn create_data_restore_driver(connection_pool: ConnectionPool) -> DataRestoreDriver {
    DataRestoreDriver::new(connection_pool, ETH_BLOCKS_DELTA, END_ETH_BLOCKS_DELTA)
}

/// Loads states from storage and start update
fn load_state() {
    let connection_pool = ConnectionPool::new();

    let mut data_restore_driver = create_data_restore_driver(connection_pool.clone());
    info!("Driver created");

    data_restore_driver
        .load_state_from_storage()
        .expect("Cant load state");
    run_state_update(&mut data_restore_driver);
}

/// Runs states updates
///
/// # Arguments
///
/// * `driver` - DataRestore Driver config
///
fn run_state_update(driver: &mut DataRestoreDriver) {
    driver.run_state_updates().expect("Cant update state");
}
