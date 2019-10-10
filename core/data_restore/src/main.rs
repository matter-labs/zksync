#[macro_use]
extern crate log;

pub mod accounts_state;
pub mod data_restore_driver;
pub mod events;
pub mod events_state;
pub mod franklin_ops;
pub mod helpers;
pub mod storage_interactor;

use crate::data_restore_driver::DataRestoreDriver;
use std::env;
use storage::ConnectionPool;
use storage_interactor::remove_storage_data;

const ETH_BLOCKS_DELTA: u64 = 250; /// Step of the considered blocks ethereum block
const END_ETH_BLOCKS_DELTA: u64 = 25; /// Delta between last ethereum block and last watched ethereum block to prevent restart in case of reorder

fn main() {
    env_logger::init();
    info!("Hello, lets build Franklin accounts state");

    let args: Vec<String> = env::args().collect();
    if args[1].clone() == format!("restart") {
        info!("Restart loading state");
        restart_state_load();
    } else if args[1].clone() == format!("continue") {
        info!("Continue loading state");
        continue_state_load();
    }
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

/// Loads state from the beginning
fn restart_state_load() {
    let connection_pool = ConnectionPool::new();

    let remove_storage_data_res = remove_storage_data(connection_pool.clone());
    if !remove_storage_data_res.is_err() {
        info!("Storage data removed");
    }

    let mut data_restore_driver = create_data_restore_driver(connection_pool.clone());
    info!("Driver created");

    run_state_update(&mut data_restore_driver);
}

/// Loads states from storage and start update
fn continue_state_load() {
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
