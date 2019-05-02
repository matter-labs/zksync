#![allow(unused_must_use)]
#![allow(deprecated)]

extern crate web3;
extern crate tiny_keccak;
extern crate tokio_core;
extern crate ethabi;

extern crate plasma;

extern crate pairing;
extern crate ff;
extern crate hex;
extern crate sapling_crypto;

pub mod events;
pub mod franklin_transaction;
pub mod blocks;
pub mod helpers;
pub mod state_builder;

#[cfg(test)]
mod test {

    use super::*;
    use web3::types::{U256, H256};
    use tokio_core::reactor::Core;

    // #[test]
    // fn test_past_and_new_events() {
    //     let mut events = events::BlockEventsFranklin::get_past_state_with_blocks_delta(helpers::InfuraEndpoint::Rinkeby, U256::from(2404)).unwrap();
    //     println!("Committed old: {:?}", events.committed_blocks);
    //     println!("Verified old: {:?}", events.verified_blocks);
    //     let mut eloop = Core::new().unwrap();
    //     events.make_new_sorted_logs_subscription(&mut eloop);
    // }

    // #[test]
    // fn test_past_events() {
    //     let events = events::BlockEventsFranklin::get_past_state_with_blocks_delta(helpers::InfuraEndpoint::Rinkeby, U256::from(300000)).unwrap();
    //     println!("Committed old: {:?}", events.committed_blocks);
    //     println!("Verified old: {:?}", events.verified_blocks);
    // }

    // #[test]
    // fn test_transactions() {
    //     let mut events = events::BlockEventsFranklin::new(helpers::InfuraEndpoint::Rinkeby);
    //     let deposit_hash = events.get_sorted_logs_in_block(U256::from(4304694)).unwrap().0[0].transaction_hash;
    //     let deposit_tx = franklin_transaction::FranklinTransaction::get_transaction(helpers::InfuraEndpoint::Rinkeby, &deposit_hash).unwrap();
    //     let exit_hash = events.get_sorted_logs_in_block(U256::from(4297243)).unwrap().0[0].transaction_hash;
    //     let exit_tx = franklin_transaction::FranklinTransaction::get_transaction(helpers::InfuraEndpoint::Rinkeby, &exit_hash).unwrap();
    //     println!("Deposit transaction: {:?}", deposit_tx);
    //     println!("Exit transaction: {:?}", exit_tx);
    // }

    // #[test]
    // fn test_build_state() {
    //     let mut events = events::BlockEventsFranklin::new(helpers::InfuraEndpoint::Rinkeby);
    //     let deposit_hash = events.get_sorted_logs_in_block(U256::from(4308277)).unwrap().0[0].transaction_hash;
    //     let deposit_tx = franklin_transaction::FranklinTransaction::get_transaction(helpers::InfuraEndpoint::Rinkeby, &deposit_hash).unwrap();
    //     let exit_hash = events.get_sorted_logs_in_block(U256::from(4308285)).unwrap().0[0].transaction_hash;
    //     let exit_tx = franklin_transaction::FranklinTransaction::get_transaction(helpers::InfuraEndpoint::Rinkeby, &exit_hash).unwrap();
    //     let mut state = state_builder::StatesBuilderFranklin::new(helpers::InfuraEndpoint::Rinkeby);
    //     let _ = state.update_accounts_states_from_transaction(&deposit_tx).unwrap();
    //     let _ = state.update_accounts_states_from_transaction(&exit_tx).unwrap();
    //     println!("Accounts states: {:?}", state.accounts_franklin);
    // }

    #[test]
    fn test_get_past_events_and_build_state() {
        let endpoint = helpers::InfuraEndpoint::Rinkeby;
        let mut state = state_builder::StatesBuilderFranklin::new(endpoint);
        let past_events = events::BlockEventsFranklin::get_past_state_with_blocks_delta(endpoint, U256::from(300000)).unwrap();
        let past_events_clone = past_events.clone();
        for v_ev in past_events_clone.verified_blocks {
            let c_ev = past_events.check_committed_block_with_same_number_as_verified(&v_ev);
            if c_ev.is_none() {
                continue;
            }
            let hash = c_ev.unwrap().transaction_hash;
            let tx = franklin_transaction::FranklinTransaction::get_transaction(endpoint, &hash).unwrap();
            let updated_state = state.update_accounts_states_from_transaction(&tx);
            if updated_state.is_err() {
                continue;
            }
        }
        println!("Accounts states: {:?}", state.accounts_franklin);
    }
}