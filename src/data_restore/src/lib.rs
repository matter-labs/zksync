#![allow(deprecated)]

extern crate web3;
extern crate tiny_keccak;
extern crate futures;
extern crate ethabi;

extern crate plasma;
// extern crate models;

extern crate pairing;
extern crate ff;
extern crate hex;
extern crate sapling_crypto;

extern crate bigdecimal;
extern crate bitvec;

pub mod block_events;
pub mod franklin_transaction;
pub mod blocks;
pub mod helpers;
pub mod accounts_state;
pub mod data_restore_driver;

use data_restore_driver::DataRestoreDriver;
// use models::{StateKeeperRequest, ProtoAccountsState};
// use std::sync::mpsc::Sender;

pub fn start_data_restore_driver(mut driver: DataRestoreDriver) {
    // let _past_state_load = driver.load_past_state().expect("Cant get past state");
    // driver.run_state_updates();
    std::thread::Builder::new().name("data_restore".to_string()).spawn(move || {
        let _past_state_load = driver.load_past_state().expect("Cant get past state");
        driver.run_state_updates();
    });
}

#[cfg(test)]
mod test {
    use super::*;

    use web3::types::{U256, H256};
    use franklin_transaction::FranklinTransaction;
    use accounts_state::FranklinAccountsStates;
    use blocks::LogBlockData;

    #[test]
    fn test_complete_task() {
        let endpoint = helpers::InfuraEndpoint::Rinkeby;
        let from = U256::from(0);
        let delta = U256::from(15);
        let mut data_restore_driver = DataRestoreDriver::new(endpoint, from, delta);
        let _past_state_load = data_restore_driver.load_past_state().expect("Cant get past state");
        data_restore_driver.run_state_updates();
    }

    #[test]
    fn test_transfer_transaction_parse() {
        let endpoint = helpers::InfuraEndpoint::Rinkeby;
        let hash = "a01852a7105d64674674ec5277b86d1e9f9016528bae2a28513be2f670a80ce6";
        let block = LogBlockData {
            block_num: 74,
            transaction_hash: H256::from(U256::from(hash)),
            block_type: blocks::BlockType::Committed
        };
        let transaction = FranklinTransaction::get_transaction(endpoint, &block).unwrap();
        let acc = FranklinAccountsStates::new(endpoint);
        let res = acc.get_all_transactions_from_transfer_block(&transaction);
        println!("{:?}", res);
    }
}