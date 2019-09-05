#[macro_use]
extern crate log;

pub mod accounts_state;
pub mod data_restore_driver;
pub mod events;
pub mod events_state;
pub mod franklin_op_block;
pub mod helpers;
pub mod storage_interactor;

use crate::data_restore_driver::DataRestoreDriver;
use std::env;
use std::str::FromStr;
use storage::ConnectionPool;
use storage_interactor::*;
use web3::types::U256;

/// Creates data restore driver state
///
/// # Arguments
///
/// * `config` - DataRestore Driver config
/// * `from` - Start ethereum block
/// * `delta` - Delta between last ethereum block and last watched ethereum block
/// * `connection_pool` - Database connection pool
///
fn create_new_data_restore_driver(
    config: helpers::DataRestoreConfig,
    from: U256,
    delta: U256,
    connection_pool: ConnectionPool,
) -> DataRestoreDriver {
    DataRestoreDriver::new(config, from, delta, connection_pool)
}

/// Loads past state
///
/// # Arguments
///
/// * `driver` - DataRestore Driver config
/// * `until_franklin_block` - Last Franklin block number for building accounts state
///
fn load_past_state_for_data_restore_driver(
    driver: &mut DataRestoreDriver,
    until_franklin_block: Option<u32>,
) {
    driver
        .load_past_state(until_franklin_block)
        .expect("Cant get past state");
}

/// Runs states ipdates
///
/// # Arguments
///
/// * `driver` - DataRestore Driver config
///
fn load_new_states_for_data_restore_driver(driver: &mut DataRestoreDriver) {
    driver.run_state_updates().expect("Cant update state");
}

/// Loads states from empty state
///
/// # Arguments
///
/// * `args` - Func Arguments
///
fn load_states_from_beginning(args: Vec<String>) {
    let config = helpers::DataRestoreConfig::new();

    let from = U256::from(0); // It's better not to allow external users to set "from block" parameter. In 99% cases 0(zero) is correct

    let delta = U256::from_dec_str(&args[1]).expect("blocks delta should be convertible to u256");
    info!("blocks delta is {}", &delta);

    let connection_pool = ConnectionPool::new();

    let remove_storage_data_res = remove_storage_data(connection_pool.clone());
    if remove_storage_data_res.is_err() {
        error!("Storage data is missing, but its not a problem");
    }

    let mut data_restore_driver =
        create_new_data_restore_driver(config, from, delta, connection_pool.clone());
    info!("Driver created");

    if args.len() > 2 {
        let until_block = u32::from_str(&args[2]).ok();
        info!("until block is {:?}", &until_block);
        load_past_state_for_data_restore_driver(&mut data_restore_driver, until_block);
    } else {
        load_past_state_for_data_restore_driver(&mut data_restore_driver, None);
        load_new_states_for_data_restore_driver(&mut data_restore_driver);
    }
}

/// Loads states from storage
///
/// # Arguments
///
/// * `args` - Func Arguments
///
fn load_states_from_storage(args: Vec<String>) {
    let connection_pool = ConnectionPool::new();

    let config = helpers::DataRestoreConfig::new();
    let from = U256::from(0); // It's better not to allow external users to set "from block" parameter. In 99% cases 0(zero) is correct
    let delta = U256::from_dec_str(&args[2]).expect("blocks delta should be convertible to u256");
    info!("blocks delta is {}", &delta);

    let mut data_restore_driver =
        create_new_data_restore_driver(config, from, delta, connection_pool.clone());
    info!("Driver created");

    if args.len() > 3 {
        let until_block = u32::from_str(&args[3]).ok();
        info!("until block is {:?}", &until_block);
        load_past_state_from_storage(
            &mut data_restore_driver,
            connection_pool.clone(),
            until_block,
        );
    } else {
        load_past_state_from_storage(&mut data_restore_driver, connection_pool.clone(), None);
        load_new_states_for_data_restore_driver(&mut data_restore_driver);
    }
}

/// Loads past states from storage
///
/// # Arguments
///
/// * `driver` - DataRestore Driver config
/// * `connection_pool` - Database connection pool
/// * `until_franklin_block` - Last Franklin block number for building accounts state
///
fn load_past_state_from_storage(
    driver: &mut DataRestoreDriver,
    connection_pool: ConnectionPool,
    until_franklin_block: Option<u32>,
) {
    info!("Loading stored state");
    driver.events_state = get_events_state_from_storage(connection_pool.clone(), driver.config.clone());
    let mut blocks = get_op_blocks_from_storage(connection_pool.clone());
    if let Some(block) = until_franklin_block {
        blocks.retain(|x| x.block_number <= block);
    }
    driver
        .update_accounts_state_from_op_blocks(blocks.as_slice())
        .expect("Cant update accounts state from op blocks in load_past_state_from_storage");
    info!("Stored state loaded");
}

fn main() {
    env_logger::init();
    info!("Hello, lets build Franklin accounts state");

    let args: Vec<String> = env::args().collect();
    if args[1].clone() == format!("storage") {
        info!("Loading states from storage");
        load_states_from_storage(args);
    } else {
        info!("Loading states from beginning");
        load_states_from_beginning(args);
    }
}
