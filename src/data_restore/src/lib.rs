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

use web3::types::U256;
use franklin_transaction::FranklinTransaction;
use helpers::*;
use block_events::BlockEventsFranklin;
use accounts_state::FranklinAccountsStates;
use blocks::LogBlockData;
// use models::{StateKeeperRequest, ProtoAccountsState};
// use std::sync::mpsc::Sender;

pub struct DataRestoreDriver {
    pub endpoint: InfuraEndpoint,
    pub genesis_block: U256,
    pub blocks_delta: U256,
    pub run_updates: bool,
    pub block_events: BlockEventsFranklin,
    pub account_states: FranklinAccountsStates,
}

pub fn start_data_restore_driver(mut driver: DataRestoreDriver) {
    let _past_state_load = driver.load_past_state().expect("Cant get past state");
    driver.run_state_updates();
    // std::thread::Builder::new().name("data_restore".to_string()).spawn(move || {
    //     let _past_state_load = driver.load_past_state().expect("Cant get past state");
    //     driver.run_state_updates();
    // });
}

impl DataRestoreDriver {

    pub fn new(endpoint: InfuraEndpoint, genesis_block: U256, blocks_delta: U256) -> Self {
        Self {
            endpoint: endpoint,
            genesis_block: genesis_block,
            blocks_delta: blocks_delta,
            run_updates: false,
            block_events: BlockEventsFranklin::new(endpoint),
            account_states: FranklinAccountsStates::new(endpoint),
        }
    }

    pub fn load_past_state(&mut self) -> Result<(), DataRestoreError> {
        let states = DataRestoreDriver::get_past_franklin_blocks_events_and_accounts_tree_state(self.endpoint, self.genesis_block, self.blocks_delta).map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        self.block_events = states.0;
        self.account_states = states.1;

        let accs = self.account_states.get_accounts();
        let root = self.account_states.root_hash();
        println!("Accs: {:?}", accs);
        println!("Root: {:?}", root);
        // let state = ProtoAccountsState {
        //     errored: false,
        //     accounts_tree: &self.account_states.accounts_tree,
        // };
        // let request = StateKeeperRequest::SetAccountsState(state);
        // let _send_result = channel.send(request).expect("Cant send through channel last state");
        Ok(())
    }

    pub fn stop_state_updates(&mut self) {
        self.run_updates = false
    }

    pub fn run_state_updates(&mut self) {
        self.run_updates = true;
        while self.run_updates {
            match DataRestoreDriver::update_franklin_blocks_events_and_accounts_tree_state(&mut self.block_events, &mut self.account_states, self.blocks_delta) {
                Err(error) => {
                    println!("Something goes wrong: {:?}", error);
                    self.run_updates = false;
                },
                Ok(()) => {
                    println!("Updated!");
                    let accs = self.account_states.get_accounts();
                    let root = self.account_states.root_hash();
                    println!("Accs: {:?}", accs);
                    println!("Root: {:?}", root);
                },
            };
            // let state = ProtoAccountsState {
            //     errored: &!self.run_updates,
            //     accounts_tree: &self.account_states.accounts_tree,
            // };
            // let request = StateKeeperRequest::SetAccountsState(state);
            // let _send_result = channel.send(request).expect("Cant send through channel last state");
        }
    }

    fn get_past_franklin_blocks_events_and_accounts_tree_state(endpoint: InfuraEndpoint, genesis_block: U256, blocks_delta: U256) -> Result<(BlockEventsFranklin, FranklinAccountsStates), DataRestoreError> {
        let events_state = DataRestoreDriver::get_past_blocks_state(endpoint, genesis_block, blocks_delta).map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        println!("Last watched block: {:?}", events_state.last_watched_block_number);
        let verified_blocks = events_state.verified_blocks.clone();
        let txs = DataRestoreDriver::get_verified_committed_blocks_transactions_from_blocks_state(&events_state, &verified_blocks);
        let sorted_txs = DataRestoreDriver::sort_transactions_by_block_number(txs);
        // println!("Transactions: {:?}", sorted_txs);

        let mut accounts_state = accounts_state::FranklinAccountsStates::new(endpoint);
        let _ = DataRestoreDriver::update_accounts_state_from_transactions(&mut accounts_state, &sorted_txs).map_err(|e| DataRestoreError::StateUpdate(e.to_string()))?;

        Ok((events_state, accounts_state))
    }

    fn get_past_blocks_state(endpoint: InfuraEndpoint, genesis_block: U256, blocks_delta: U256) -> Result<BlockEventsFranklin, DataRestoreError> {
        let events = block_events::BlockEventsFranklin::get_past_state_from_genesis_with_blocks_delta(endpoint, genesis_block, blocks_delta).map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        Ok(events)
    }

    fn get_verified_committed_blocks_transactions_from_blocks_state(block_events_state: &BlockEventsFranklin, verified_blocks: &Vec<LogBlockData>) -> Vec<FranklinTransaction> {
        let committed_blocks = block_events_state.get_only_verified_committed_blocks(verified_blocks);
        println!("Committed verified blocks: {:?}", committed_blocks);
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

    fn sort_transactions_by_block_number(transactions: Vec<FranklinTransaction>) -> Vec<FranklinTransaction> {
        let mut sorted_transactions = transactions;
        sorted_transactions.sort_by_key(|x| x.block_number);
        sorted_transactions
    }

    fn update_accounts_state_from_transactions(state: &mut accounts_state::FranklinAccountsStates, transactions: &Vec<FranklinTransaction>) -> Result<(), DataRestoreError> {
        // let mut state = accounts_state::FranklinAccountsStates::new(endpoint);
        for transaction in transactions {
            let _ = state.update_accounts_states_from_transaction(&transaction).map_err(|e| DataRestoreError::StateUpdate(e.to_string()))?;
        }
        Ok(())
    }

    fn update_franklin_blocks_events_and_accounts_tree_state(block_events_state: &mut BlockEventsFranklin, accounts_state: &mut FranklinAccountsStates, blocks_delta: U256) -> Result<(), DataRestoreError> {
        let new_events = block_events_state.update_state_from_last_watched_block_with_blocks_delta_and_return_new_blocks(blocks_delta).map_err(|e| DataRestoreError::StateUpdate(e.to_string()))?;
        // if new_events.is_err() {
        //     // Ok(())
        // }
        println!("Last watched block: {:?}", &block_events_state.last_watched_block_number);
        if new_events.1.is_empty() {
            return Err(DataRestoreError::NoData("No verified blocks".to_string()))
            // Ok(())
        }
        let verified_blocks = &new_events.1;
        let txs = DataRestoreDriver::get_verified_committed_blocks_transactions_from_blocks_state(&block_events_state, &verified_blocks);
        let sorted_txs = DataRestoreDriver::sort_transactions_by_block_number(txs);

        let _ = DataRestoreDriver::update_accounts_state_from_transactions(accounts_state, &sorted_txs).map_err(|e| DataRestoreError::StateUpdate(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_complete_task() {
        let endpoint = helpers::InfuraEndpoint::Rinkeby;
        let from = U256::from(0);
        let delta = U256::from(15);
        let data_restore_driver = DataRestoreDriver::new(endpoint, from, delta);
        start_data_restore_driver(data_restore_driver);
    }

    use web3::types::{H256, U256};

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