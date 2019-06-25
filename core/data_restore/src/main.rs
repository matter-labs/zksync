#[macro_use]
extern crate log;

pub mod accounts_state;
pub mod block_events;
pub mod blocks;
pub mod data_restore_driver;
pub mod franklin_transaction;
pub mod helpers;

use crate::data_restore_driver::DataRestoreDriver;
use std::env;
use std::str::FromStr;
use web3::types::U256;

pub fn create_new_data_restore_driver(
    config: helpers::DataRestoreConfig,
    from: U256,
    delta: U256,
) -> DataRestoreDriver {
    DataRestoreDriver::new(config, from, delta)
}

pub fn load_past_state_for_data_restore_driver(driver: &mut DataRestoreDriver) {
    driver.load_past_state().expect("Cant get past state");
}

pub fn load_new_states_for_data_restore_driver(driver: &mut DataRestoreDriver) {
    driver.run_state_updates().expect("Cant update state");
}

// pub fn load_new_states_for_data_restore_driver(driver: &'static mut DataRestoreDriver) {
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

fn main() {
    env_logger::init();
    info!("Hello, lets build Franklin accounts state");

    let args: Vec<String> = env::args().collect();

    let infura_endpoint_id = u8::from_str(&args[1]).expect("Network endpoint should be convertible to u8");
    info!("Network number is {}", &infura_endpoint_id);
    let config = match infura_endpoint_id {
        1 => Some(helpers::DataRestoreConfig::new(helpers::InfuraEndpoint::Mainnet)),
        4 => Some(helpers::DataRestoreConfig::new(helpers::InfuraEndpoint::Rinkeby)),
        _ => None,
    }.expect("It's acceptable only 1 for Mainnet and 4 for Rinkeby networks");
    let from = U256::from(0); // It's better not to allow external users to set "from block" parameter. In 99% cases 0(zero) is correct
    let delta = U256::from_str(&args[2]).expect("Blocks delta should be convertible to u256");
    info!("Blocks delta is {}", &delta);

    let mut data_restore_driver = create_new_data_restore_driver(config, from, delta);
    load_past_state_for_data_restore_driver(&mut data_restore_driver);
    load_new_states_for_data_restore_driver(&mut data_restore_driver);
}



#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_complete_task() {
        let config = helpers::DataRestoreConfig::new(helpers::InfuraEndpoint::Rinkeby);
        let from = U256::from(0);
        let delta = U256::from(15);
        let mut data_restore_driver = create_new_data_restore_driver(config, from, delta);
        data_restore_driver
            .load_past_state()
            .expect("Cant get past state");
        data_restore_driver.run_state_updates();
    }
}
