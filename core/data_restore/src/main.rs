#[macro_use]
extern crate log;

pub mod accounts_state;
pub mod node_restore_driver;
pub mod events;
pub mod events_state;
pub mod franklin_ops;
pub mod genesis_state;
pub mod helpers;
pub mod storage_interactor;

use crate::node_restore_driver::NodeRestoreDriver;
use storage::ConnectionPool;

fn main() {
    env_logger::init();
    info!("Building Franklin accounts state");
    load_state();
}

/// Creates node restore driver state
///
/// # Arguments
///
/// * `connection_pool` - Database connection pool
///
fn create_node_restore_driver(connection_pool: ConnectionPool) -> NodeRestoreDriver {
    NodeRestoreDriver::new(connection_pool)
}

/// Loads states from storage and start update
fn load_state() {
    let connection_pool = ConnectionPool::new();

    let mut node_restore_driver = create_node_restore_driver(connection_pool.clone());
    info!("Driver created");

    node_restore_driver
        .load_state_from_storage()
        .expect("Cant load state");
    run_state_update(&mut node_restore_driver);
}

/// Runs states updates
///
/// # Arguments
///
/// * `driver` - NodeRestore Driver config
///
fn run_state_update(driver: &mut NodeRestoreDriver) {
    driver.run_state_updates().expect("Cant update state");
}
