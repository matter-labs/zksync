#![allow(deprecated)]

extern crate web3;
extern crate tiny_keccak;
extern crate futures;
extern crate ethabi;

extern crate plasma;

extern crate pairing;
extern crate ff;
extern crate hex;
extern crate sapling_crypto;

extern crate bigdecimal;

pub mod events;
pub mod franklin_transaction;
pub mod blocks;
pub mod helpers;
pub mod state_builder;

#[cfg(test)]
mod test {

    use super::*;
    use web3::types::U256;

    // #[test]
    // fn test_past_and_new_events() {
    //     let mut events = events::BlockEventsFranklin::get_past_state_from_genesis_with_blocks_delta(helpers::InfuraEndpoint::Rinkeby, U256::from(2404)).unwrap();
    //     println!("Committed old: {:?}", events.committed_blocks);
    //     println!("Verified old: {:?}", events.verified_blocks);
    //     let mut eloop = Core::new().unwrap();
    //     events.make_new_sorted_logs_subscription(&mut eloop);
    // }

    // #[test]
    // fn test_build_state_deposit_exit() {
    //     let mut events = events::BlockEventsFranklin::new(helpers::InfuraEndpoint::Rinkeby);
    //     let deposit_hash = events.get_sorted_logs_in_block(U256::from(4308277)).unwrap().0[0].transaction_hash;
    //     let deposit_tx = franklin_transaction::FranklinTransaction::get_transaction(helpers::InfuraEndpoint::Rinkeby, &deposit_hash).unwrap();
    //     let exit_hash = events.get_sorted_logs_in_block(U256::from(4308285)).unwrap().0[0].transaction_hash;
    //     let exit_tx = franklin_transaction::FranklinTransaction::get_transaction(helpers::InfuraEndpoint::Rinkeby, &exit_hash).unwrap();
    //     let mut state = state_builder::StatesBuilderFranklin::new(helpers::InfuraEndpoint::Rinkeby);
    //     let _ = state.update_accounts_states_from_transaction(&deposit_tx).unwrap();
    //     let _ = state.update_accounts_states_from_transaction(&exit_tx).unwrap();
    //     let accs = state.get_accounts();
    //     println!("Accounts: {:?}", accs);
    // }

    // #[test]
    // fn test_build_state_deposit_transfer() {
    //     let mut events = events::BlockEventsFranklin::new(helpers::InfuraEndpoint::Rinkeby);
    //     let deposit_hash = events.get_sorted_logs_in_block(U256::from(4313380)).unwrap().0[0].transaction_hash;
    //     let deposit_tx = franklin_transaction::FranklinTransaction::get_transaction(helpers::InfuraEndpoint::Rinkeby, &deposit_hash).unwrap();
    //     let transfer_hash = events.get_sorted_logs_in_block(U256::from(4313487)).unwrap().0[0].transaction_hash;
    //     let transfer_tx = franklin_transaction::FranklinTransaction::get_transaction(helpers::InfuraEndpoint::Rinkeby, &transfer_hash).unwrap();
    //     let mut state = state_builder::StatesBuilderFranklin::new(helpers::InfuraEndpoint::Rinkeby);
    //     let _ = state.update_accounts_states_from_transaction(&deposit_tx).unwrap();
    //     let _ = state.update_accounts_states_from_transaction(&transfer_tx).unwrap();
    //     let accs = state.get_accounts();
    //     println!("Accounts: {:?}", accs);
    // }

    #[test]
    fn test_get_past_events_and_build_state() {
        let endpoint = helpers::InfuraEndpoint::Rinkeby;
        let mut state = state_builder::StatesBuilderFranklin::new(endpoint);
        let events = events::BlockEventsFranklin::get_past_state_from_genesis_with_blocks_delta(endpoint, U256::from(3972344), U256::from(15)).unwrap();
        let events_clone = events.clone();
        for v_ev in events_clone.verified_blocks {
            let c_ev = events.check_committed_block_with_same_number_as_verified(&v_ev);
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
        let accs = state.get_accounts();
        println!("Accounts: {:?}", accs);
        let root = state.root_hash();
        println!("Root: {:?}", root);
    }
}