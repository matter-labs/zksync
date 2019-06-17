pub mod accounts_state;
pub mod block_events;
pub mod blocks;
pub mod data_restore_driver;
pub mod franklin_transaction;
pub mod helpers;

use crate::data_restore_driver::{DataRestoreDriver, ProtoAccountsState};
use std::sync::mpsc::Sender;
use web3::types::U256;

pub fn create_new_data_restore_driver(
    config: helpers::DataRestoreConfig,
    from: U256,
    delta: U256,
    channel: Option<Sender<ProtoAccountsState>>,
) -> DataRestoreDriver {
    DataRestoreDriver::new(config, from, delta, channel)
}

pub fn load_past_state_for_data_restore_driver(driver: &mut DataRestoreDriver) {
    driver.load_past_state().expect("Cant get past state");
}

pub fn load_new_states_for_data_restore_driver(driver: &'static mut DataRestoreDriver) {
    std::thread::Builder::new()
        .name("data_restore".to_string())
        .spawn(move || {
            let _ = driver.run_state_updates().expect("Cant update state");
        });
}

pub fn start_data_restore_driver(driver: &'static mut DataRestoreDriver) {
    std::thread::Builder::new()
        .name("data_restore".to_string())
        .spawn(move || {
            let _past_state_load = driver.load_past_state().expect("Cant get past state");
            let _ = driver.run_state_updates().expect("Cant update state");
        });
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_complete_task() {
        let config = helpers::DataRestoreConfig::new(helpers::InfuraEndpoint::Rinkeby);
        let from = U256::from(0);
        let delta = U256::from(15);
        let mut data_restore_driver = create_new_data_restore_driver(config, from, delta, None);
        let _past_state_load = data_restore_driver
            .load_past_state()
            .expect("Cant get past state");
        data_restore_driver.run_state_updates();
    }
}
