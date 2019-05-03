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

pub mod block_events;
pub mod franklin_transaction;
pub mod blocks;
pub mod helpers;
pub mod accounts_state;

use web3::types::U256;
use franklin_transaction::FranklinTransaction;
use helpers::InfuraEndpoint;
use block_events::BlockEventsFranklin;
use accounts_state::FranklinAccountsStates;

pub fn get_past_blocks_state(endpoint: InfuraEndpoint, genesis_block: U256, blocks_delta: U256) -> Result<BlockEventsFranklin, String> {
    let events = block_events::BlockEventsFranklin::get_past_state_from_genesis_with_blocks_delta(endpoint, genesis_block, blocks_delta);
    if events.is_err() {
        return Err(String::from("Cant get past events"));
    }
    Ok(events.unwrap())
}

pub fn get_committed_blocks_transactions_from_blocks_state(block_events_state: &BlockEventsFranklin) -> Vec<FranklinTransaction> {
    let committed_blocks = block_events_state.get_only_verified_committed_blocks();
    let mut transactions = vec![];
    for block in committed_blocks {
        let tx = FranklinTransaction::get_transaction(block_events_state.endpoint, &block);
        if tx.is_none() {
            continue;
        }
        transactions.push(tx.unwrap());
    }
    transactions
}

pub fn sort_transactions_by_block_number(transactions: Vec<FranklinTransaction>) -> Vec<FranklinTransaction> {
    let mut sorted_transactions = transactions;
    sorted_transactions.sort_by_key(|x| x.block_number);
    sorted_transactions
}

pub fn get_franklin_accounts_state_from_past_transactions(endpoint: InfuraEndpoint, transactions: &Vec<FranklinTransaction>) -> Result<FranklinAccountsStates, String> {
    let mut state = accounts_state::FranklinAccountsStates::new(endpoint);
    for transaction in transactions {
        let updated_state = state.update_accounts_states_from_transaction(&transaction);
        if updated_state.is_err() {
            return Err(String::from("Cant update state from transaction"))
        }
    }
    Ok(state)
}

pub fn get_franklin_blocks_events_and_accounts_tree_state(endpoint: InfuraEndpoint, genesis_block: U256, blocks_delta: U256) -> Result<(BlockEventsFranklin, FranklinAccountsStates), String> {
    let events_state = get_past_blocks_state(endpoint, genesis_block, blocks_delta);
    if events_state.is_err() {
        return Err(String::from("Cant get past blocks state"))
    }
    let unwraped_events_state = events_state.unwrap();
    println!("Last watched block: {:?}", unwraped_events_state.last_watched_block_number);
    println!("Committed blocks: {:?}", unwraped_events_state.committed_blocks);
    println!("Verified blocks: {:?}", unwraped_events_state.committed_blocks);

    let txs = get_committed_blocks_transactions_from_blocks_state(&unwraped_events_state);
    let sorted_txs = sort_transactions_by_block_number(txs);
    println!("Transactions: {:?}", sorted_txs);

    let accounts_state = get_franklin_accounts_state_from_past_transactions(endpoint, &sorted_txs);
    if accounts_state.is_err() {
        return Err(String::from("Cant get past accounts state"))
    }
    let unwraped_accounts_state = accounts_state.unwrap();

    Ok((unwraped_events_state, unwraped_accounts_state))
}

pub fn update_franklin_blocks_events_and_accounts_tree_state(block_events_state: &mut BlockEventsFranklin, accounts_state: &mut FranklinAccountsStates, blocks_delta: U256) -> Result<(), String> {
    let new_events = block_events_state.update_state_from_last_watched_block_with_blocks_delta_and_return_new_blocks(blocks_delta);
    if new_events.is_err() {
        return Err(String::from("Cant get new blocks"))
    }
    let unwraped_new_events = new_events.unwrap();

    for verified_event in unwraped_new_events.1 {
        let committed_event = block_events_state.check_committed_block_with_same_number_as_verified(&verified_event);
        if committed_event.is_none() {
            return Err(String::from("Cant get committed event"))
        }
        let unwraped_committed_event = committed_event.unwrap();

        let new_tx = franklin_transaction::FranklinTransaction::get_transaction(block_events_state.endpoint, &unwraped_committed_event);
        if new_tx.is_none() {
            return Err(String::from("Cant get transaction"))
        }
        let unwraped_tx = new_tx.unwrap();

        let updated_state = accounts_state.update_accounts_states_from_transaction(&unwraped_tx);
        if updated_state.is_err() {
            return Err(String::from("Cant update accounts"))
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_complete_task() {
        // get past events
        let endpoint = helpers::InfuraEndpoint::Rinkeby;
        let from = U256::from(3972344);
        let delta = U256::from(15);
        let states = get_franklin_blocks_events_and_accounts_tree_state(endpoint, from, delta);
        if states.is_err() {
            panic!("Cant get past blocks state");
        }
        let unwraped_states = states.unwrap();
        let mut block_events = unwraped_states.0;
        let mut accounts = unwraped_states.1;

        let mut accs = accounts.get_accounts();
        println!("Accounts: {:?}", accs);
        let mut root = accounts.root_hash();
        println!("Root: {:?}", root);

        // getting new events
        match update_franklin_blocks_events_and_accounts_tree_state(&mut block_events, &mut accounts, delta) {
            Err(error) => {
                println!("Something goes wrong: {:?}", error);
            },
            Ok(()) => {
                accs = accounts.get_accounts();
                println!("Accounts: {:?}", accs);
                root = accounts.root_hash();
                println!("Root: {:?}", root);
            },
        };
    }
}