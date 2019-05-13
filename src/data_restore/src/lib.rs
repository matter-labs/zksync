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
use helpers::*;
use block_events::BlockEventsFranklin;
use accounts_state::FranklinAccountsStates;
use blocks::LogBlockData;

pub struct DataRestoreDriver {
    pub endpoint: InfuraEndpoint,
    pub genesis_block: U256,
    pub blocks_delta: U256,
    pub run_updates: bool,
    pub block_events: BlockEventsFranklin,
    pub account_states: FranklinAccountsStates,
}

impl DataRestoreDriver {
    pub fn get_past_state(endpoint: InfuraEndpoint, genesis_block: U256, blocks_delta: U256) -> Result<Self, DataRestoreError> {
        let states = DataRestoreDriver::get_past_franklin_blocks_events_and_accounts_tree_state(endpoint, genesis_block, blocks_delta).map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        let block_events = states.0;
        let account_states = states.1;

        let this = Self {
            endpoint: endpoint,
            genesis_block: genesis_block,
            blocks_delta: blocks_delta,
            run_updates: true,
            block_events: block_events,
            account_states: account_states,
        };
        Ok(this)
    }

    // TODO : - need to make this async
    pub fn run_state_updates(&mut self) {
        while self.run_updates {
            match DataRestoreDriver::update_franklin_blocks_events_and_accounts_tree_state(&mut self.block_events, &mut self.account_states, self.blocks_delta) {
                Err(error) => {
                    println!("Something goes wrong: {:?}", error);
                },
                Ok(()) => {
                    println!("Updated!");
                    println!("Accounts: {:?}", self.account_states.get_accounts());
                    println!("Root: {:?}", self.account_states.root_hash());
                },
            };
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
        println!("Last watched block: {:?}", &block_events_state.last_watched_block_number);
        if new_events.1.is_empty() {
            return Err(DataRestoreError::NoData("No verified blocks".to_string()))
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
        // get past events
        let endpoint = helpers::InfuraEndpoint::Rinkeby;
        let from = U256::from(3972344);
        let delta = U256::from(21095);
        let data_restore_driver = DataRestoreDriver::get_past_state(endpoint, from, delta);
        if data_restore_driver.is_err() {
            panic!("Cant get past state");
        }
        let mut unwraped_data_restore_driver = data_restore_driver.unwrap();
        let driver = &mut unwraped_data_restore_driver;
        {
            let accounts_states = &driver.account_states;
            let accs = accounts_states.get_accounts();
            println!("Accounts: {:?}", accs);
            let root = accounts_states.root_hash();
            println!("Root: {:?}", root);
        }
        driver.run_state_updates();
    }
}