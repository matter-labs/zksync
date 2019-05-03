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
pub mod state_builder;

use web3::types::U256;
use blocks::LogBlockData;
use franklin_transaction::{FranklinTransaction, FranklinTransactionType};
use helpers::InfuraEndpoint;
use block_events::BlockEventsFranklin;
use state_builder::StatesBuilderFranklin;

pub fn get_past_blocks_state(endpoint: InfuraEndpoint, genesis_block: U256, blocks_delta: U256) -> Result<BlockEventsFranklin, String> {
    let events = block_events::BlockEventsFranklin::get_past_state_from_genesis_with_blocks_delta(endpoint, genesis_block, blocks_delta);
    if events.is_err() {
        return Err(String::from("Cant get past events"));
    }
    Ok(events.unwrap())
}

pub fn get_committed_blocks_transactions_from_state(block_events_state: &BlockEventsFranklin) -> Vec<FranklinTransaction> {
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

pub fn get_franklin_state_from_past_transactions(endpoint: InfuraEndpoint, transactions: &Vec<FranklinTransaction>) -> Result<StatesBuilderFranklin, String> {
    let mut state = state_builder::StatesBuilderFranklin::new(endpoint);
    for transaction in transactions {
        let updated_state = state.update_accounts_states_from_transaction(&transaction);
        if updated_state.is_err() {
            return Err(String::from("Cant update state from transaction"))
        }
    }
    Ok(state)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_complete_task() {
        let endpoint = helpers::InfuraEndpoint::Rinkeby;
        let events_state = get_past_blocks_state(endpoint, U256::from(3972344), U256::from(15));
        if events_state.is_err() {
            panic!("Cant get past blocks state");
        }
        let unwraped_events_state = events_state.unwrap();
        println!("State: {:?}", unwraped_events_state);
        let txs = get_committed_blocks_transactions_from_state(&unwraped_events_state);
        let sorted_txs = sort_transactions_by_block_number(txs);
        println!("Transactions: {:?}", sorted_txs);
        let franklin_state = get_franklin_state_from_past_transactions(endpoint, &sorted_txs);
        if franklin_state.is_err() {
            panic!("Cant build franklin state");
        }
        let unwraped_franklin_state = franklin_state.unwrap();
        let accs = unwraped_franklin_state.get_accounts();
        println!("Accounts: {:?}", accs);
        let root = unwraped_franklin_state.root_hash();
        println!("Root: {:?}", root);
    }
}