#[macro_use]
extern crate log;

pub mod accounts_state;
pub mod events_state;
pub mod events;
pub mod data_restore_driver;
pub mod franklin_transaction;
pub mod helpers;
pub mod storage_interactor;

use crate::data_restore_driver::DataRestoreDriver;
use std::env;
use std::str::FromStr;
use storage::ConnectionPool;
use storage_interactor::*;
use web3::types::U256;

fn create_new_data_restore_driver(
    config: helpers::DataRestoreConfig,
    from: U256,
    delta: U256,
    connection_pool: ConnectionPool,
) -> DataRestoreDriver {
    DataRestoreDriver::new(config, from, delta, connection_pool)
}

fn load_past_state_for_data_restore_driver(driver: &mut DataRestoreDriver, until_franklin_block: Option<u32>) {
    driver.load_past_state(until_franklin_block).expect("Cant get past state");
}

fn load_new_states_for_data_restore_driver(driver: &mut DataRestoreDriver) {
    driver.run_state_updates().expect("Cant update state");
}

// pub fn load_new_states_for_stored_data_restore_driver() {
//     std::thread::Builder::new()
//         .name("data_restore".to_string())
//         .spawn(move || {
//             driver.run_state_updates().expect("Cant update state");
//         })
//         .expect("Load new states for data restore thread");
// }

// pub fn start_data_restore_driver(driver: &'static mut DataRestoreDriver) {
//     std::thread::Builder::new()
//         .name("data_restore".to_string())
//         .spawn(move || {
//             driver.load_past_state().expect("Cant get past state");
//             driver.run_state_updates().expect("Cant update state");
//         })
//         .expect("Data restore driver thread");
// }

fn load_states_from_beginning(args: Vec<String>) {
    let infura_endpoint_id =
        u8::from_str(&args[1]).expect("Network endpoint should be convertible to u8");
    info!("Network number is {}", &infura_endpoint_id);
    let config = match infura_endpoint_id {
        1 => Some(helpers::DataRestoreConfig::new(
            helpers::InfuraEndpoint::Mainnet,
        )),
        4 => Some(helpers::DataRestoreConfig::new(
            helpers::InfuraEndpoint::Rinkeby,
        )),
        _ => None,
    }
    .expect("It's acceptable only 1 for Mainnet and 4 for Rinkeby networks");

    let from = U256::from(0); // It's better not to allow external users to set "from block" parameter. In 99% cases 0(zero) is correct
    
    let delta = U256::from_dec_str(&args[2]).expect("blocks delta should be convertible to u256");
    info!("blocks delta is {}", &delta);

    let connection_pool = ConnectionPool::new();

    let remove_storage_data_res = remove_storage_data(connection_pool.clone());
    if remove_storage_data_res.is_err() {
        error!("Storage data is missing, but its not a problem");
    }

    let mut data_restore_driver =
        create_new_data_restore_driver(config, from, delta, connection_pool.clone());
    info!("Driver created");

    if args.len() > 3 {
        let until_block = u32::from_str(&args[3]).ok();
        info!("until block is {:?}", &until_block);
        load_past_state_for_data_restore_driver(&mut data_restore_driver, until_block);
    } else {
        load_past_state_for_data_restore_driver(&mut data_restore_driver, None);
        load_new_states_for_data_restore_driver(&mut data_restore_driver);
    }
}

fn load_states_from_storage(args: Vec<String>) {
    let connection_pool = ConnectionPool::new();

    let config =
        get_config_from_storage(connection_pool.clone()).expect("Network id is broken in storage");
    let from = U256::from(0); // It's better not to allow external users to set "from block" parameter. In 99% cases 0(zero) is correct
    let delta = U256::from_dec_str(&args[2]).expect("blocks delta should be convertible to u256");
    info!("blocks delta is {}", &delta);

    let mut data_restore_driver =
        create_new_data_restore_driver(config, from, delta, connection_pool.clone());
    info!("Driver created");

    if args.len() > 3 {
        let until_block = u32::from_str(&args[3]).ok();
        info!("until block is {:?}", &until_block);
        load_past_state_from_storage(&mut data_restore_driver, connection_pool.clone(), until_block);
    } else {
        load_past_state_from_storage(&mut data_restore_driver, connection_pool.clone(), None);
        load_new_states_for_data_restore_driver(&mut data_restore_driver);
    }
}

fn load_past_state_from_storage(driver: &mut DataRestoreDriver, connection_pool: ConnectionPool, until_franklin_block: Option<u32>) {
    info!("Loading stored state");
    driver.events_state = get_events_state_from_storage(connection_pool.clone());
    let mut transactions = get_transactions_from_storage(connection_pool.clone());
    if let Some(block) = until_franklin_block {
        transactions.retain(|x| x.block_number <= block);
    }
    driver
        .update_accounts_state_from_transactions(transactions.as_slice())
        .expect("Cant update accounts state from transactions in load_past_state_from_storage");
    // for tx in transactions {
    //     driver
    //         .account_states
    //         .update_accounts_states_from_transaction(&tx)
    //         .expect("Cant update accounts state");
    // }
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

// #[cfg(test)]
// mod test {
//     use super::*;

//     #[test]
//     fn test_complete_task() {
//         let config = helpers::DataRestoreConfig::new(helpers::InfuraEndpoint::Rinkeby);
//         let from = U256::from(0);
//         let delta = U256::from(15);
//         let mut data_restore_driver = create_new_data_restore_driver(config, from, delta);
//         data_restore_driver
//             .load_past_state()
//             .expect("Cant get past state");
//         data_restore_driver.run_state_updates();
//     }
// }
