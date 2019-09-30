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
// use std::str::FromStr;
use storage::ConnectionPool;
use storage_interactor::{
    remove_storage_data,
    // get_events_state_from_storage,
    // get_op_blocks_from_storage
};
// use web3::types::U256;

fn main() {
    env_logger::init();
    info!("Hello, lets build Franklin accounts state");

    let args: Vec<String> = env::args().collect();
    if args[1].clone() == format!("restart") {
        info!("Restart loading state");
        restart_state_load(args);
    } else {
        info!("Continue loading state");
        continue_state_load(args);
    }
}

/// Creates data restore driver state
///
/// # Arguments
///
/// * `connection_pool` - Database connection pool
///
fn create_data_restore_driver(
    connection_pool: ConnectionPool,
) -> DataRestoreDriver {
    DataRestoreDriver::new(connection_pool, 12, 250, 25) // TODO: - rethinks how to get blocks
}

/// Loads state from the beginning
///
/// # Arguments
///
/// * `args` - Func Arguments
///
fn restart_state_load(args: Vec<String>) {
    let connection_pool = ConnectionPool::new();

    let remove_storage_data_res = remove_storage_data(connection_pool.clone());
    if !remove_storage_data_res.is_err() {
        info!("Storage data removed");
    }

    let mut data_restore_driver =
        create_data_restore_driver(connection_pool.clone());
    info!("Driver created");

    run_state_update(&mut data_restore_driver);
}

/// Loads states from storage and start update
///
/// # Arguments
///
/// * `args` - Func Arguments
///
fn continue_state_load(args: Vec<String>) {
    let connection_pool = ConnectionPool::new();

    let mut data_restore_driver =
        create_data_restore_driver(connection_pool.clone());
    info!("Driver created");

    // data_restore_driver.load_state_from_storage();
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
