use crate::accounts_state::FranklinAccountsStates;
use crate::events::EventData;
use crate::events_state::EventsState;
use crate::franklin_transaction::FranklinTransaction;
use crate::helpers::*;
use crate::storage_interactor;
use std::fs::File;
use std::io::{BufWriter, Write};
use storage::ConnectionPool;
use web3::types::U256;

const FILENAME: &str = "restored_data.txt";

pub struct DataRestoreDriver {
    pub connection_pool: ConnectionPool,
    pub config: DataRestoreConfig,
    pub genesis_block: U256,
    pub blocks_delta: U256,
    pub run_updates: bool,
    pub events_state: EventsState,
    pub account_states: FranklinAccountsStates,
    pub transactions: Vec<FranklinTransaction>,
}

impl DataRestoreDriver {
    pub fn new(
        config: DataRestoreConfig,
        genesis_block: U256,
        blocks_delta: U256,
        connection_pool: ConnectionPool,
    ) -> Self {
        Self {
            connection_pool,
            config: config.clone(),
            genesis_block,
            blocks_delta,
            run_updates: false,
            events_state: EventsState::new(config.clone()),
            account_states: FranklinAccountsStates::new(config.clone()),
            transactions: vec![],
        }
    }

    pub fn load_past_state(&mut self, until_block: Option<u32>) -> Result<(), DataRestoreError> {
        info!("Loading past state");
        self.update_past_franklin_blocks_events_and_accounts_tree_state(until_block)
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        // self.events_state = states.0;
        // self.account_states = states.1;

        // let accs = &self.account_states.get_accounts();
        // debug!("Accs: {:?}", accs);

        self.save_complete_storage_state();

        info!("Finished loading past state");
        Ok(())
    }

    // pub fn get_past_state_until_block(&mut self, block_num: u32) -> Result<(), DataRestoreError> {

    //     // TODO: pop transactions with block number > block_num

    //     info!("Loading past state until block: {}", block_num);
    //     let mut verified_blocks = self.events_state.verified_blocks.clone();
    //     if verified_blocks.len() > block_num {
    //         verified_blocks.pop(verified_blocks.len() - block_num);
    //     }

    //     let txs =
    //         self.get_verified_committed_blocks_transactions_from_blocks_state(&verified_blocks);
    //     let sorted_txs = DataRestoreDriver::sort_transactions_by_block_number(txs);
    //     // debug!("Transactions: {:?}", sorted_txs);

    //     self.account_states = FranklinAccountsStates::new(self.config.clone());
    //     self.update_accounts_state_from_transactions(&sorted_txs)
    //         .map_err(|e| DataRestoreError::StateUpdate(e.to_string()))?;

    //     info!("Finished loading past state");
    //     Ok(())
    // }

    pub fn stop_state_updates(&mut self) {
        self.run_updates = false
    }

    pub fn run_state_updates(&mut self) -> Option<DataRestoreError> {
        info!("Start state updates");
        self.run_updates = true;
        let mut err: Option<DataRestoreError> = None;
        while self.run_updates {
            info!(
                "Last watched ethereum block: {:?}",
                &self.events_state.last_watched_block_number
            );
            info!(
                "Committed franklin blocks count: {:?}",
                &self.events_state.committed_blocks.len()
            );
            info!(
                "Verified franklin blocks count: {:?}",
                &self.events_state.verified_blocks.len()
            );
            // match DataRestoreDriver::update_franklin_blocks_events_and_accounts_tree_state(self) {
            //     Err(error) => {
            //         error!("Something goes wrong: {:?}", error);
            //         self.run_updates = false;
            //         err = Some(DataRestoreError::StateUpdate(format!(
            //             "Error occured: {:?}",
            //             error
            //         )));
            //     }
            //     Ok(()) => {
            //         // debug!("Updated, last watched ethereum block: {:?}", &self.events_state.last_watched_block_number);
            //         // debug!("Committed franklin blocks count: {:?}", &self.events_state.committed_blocks.len());
            //         // debug!("Last committed franklin block: {:?}", &self.events_state.committed_blocks.last());
            //         // debug!("Verified franklin blocks count: {:?}", &self.events_state.verified_blocks.len());
            //         // debug!("Last verified franklin block: {:?}", &self.events_state.verified_blocks.last());
            //         // let accs = self.account_states.get_accounts();
            //         // let root = self.account_states.root_hash();
            //         // debug!("Accs: {:?}", accs);
            //         // debug!("Root: {:?}", &root);
            //     }
            // };
            if let Err(error) = self.update_franklin_blocks_events_and_accounts_tree_state() {
                error!("Something goes wrong: {:?}", error);
                self.run_updates = false;
                err = Some(DataRestoreError::StateUpdate(format!(
                    "Error occured: {:?}",
                    error
                )));
            }
        }
        info!("Stopped state updates");
        err
    }

    fn update_past_franklin_blocks_events_and_accounts_tree_state(
        &mut self,
        until_block: Option<u32>,
    ) -> Result<(), DataRestoreError> {
        let mut got_events = false;
        while !got_events {
            if let Ok(()) = self.update_past_blocks_events_state() {
                got_events = true;
            }
        }
        // debug!("Last watched block: {:?}", events_state.last_watched_block_number);
        let verified_blocks = self.events_state.verified_blocks.clone();

        let txs =
            self.get_verified_committed_blocks_transactions_from_blocks_state(&verified_blocks);
        let mut sorted_txs = DataRestoreDriver::sort_transactions_by_block_number(txs);

        self.transactions.append(&mut sorted_txs.clone());

        self.account_states = FranklinAccountsStates::new(self.config.clone());

        if let Some(block) = until_block {
            sorted_txs.retain(|x| x.block_number <= block);
        }
        self.update_accounts_state_from_transactions(&sorted_txs)
            .map_err(|e| DataRestoreError::StateUpdate(e.to_string()))?;

        Ok(())
    }

    fn save_complete_storage_state(&mut self) {
        info!("Saving storage state");
        let mut logs = self.events_state.committed_blocks.clone();
        logs.append(&mut self.events_state.verified_blocks.clone());

        storage_interactor::save_tree_restore_from_config(
            &self.config,
            self.connection_pool.clone(),
        );
        storage_interactor::save_events_state(&logs, self.connection_pool.clone());
        storage_interactor::save_last_watched_block_number(
            &mut self.events_state.last_watched_block_number,
            self.connection_pool.clone(),
        );
        storage_interactor::save_franklin_transactions(
            &self.transactions,
            self.connection_pool.clone(),
        );
        info!("Storage state saved");
    }

    fn update_storage_state(&mut self, logs: &Vec<EventData>, txs: &Vec<FranklinTransaction>) {
        info!("Updating storage state");
        storage_interactor::save_events_state(&logs, self.connection_pool.clone());
        storage_interactor::save_last_watched_block_number(
            &self.events_state.last_watched_block_number,
            self.connection_pool.clone(),
        );
        storage_interactor::save_franklin_transactions(txs, self.connection_pool.clone());
        info!("Storage state updated");
    }

    fn update_past_blocks_events_state(&mut self) -> Result<(), DataRestoreError> {
        info!("Loading events");
        let events = EventsState::get_past_state_from_genesis_with_blocks_delta(
            self.config.clone(),
            self.genesis_block.clone(),
            self.blocks_delta.clone(),
        )
        .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        info!(
            "Got past events state till ethereum block: {:?}",
            &events.last_watched_block_number
        );
        info!(
            "Committed franklin blocks count: {:?}",
            &events.committed_blocks.len()
        );
        debug!(
            "Last committed franklin block: {:?}",
            &events.committed_blocks.last()
        );
        info!(
            "Verified franklin blocks count: {:?}",
            &events.verified_blocks.len()
        );
        debug!(
            "Last verified franklin block: {:?}",
            &events.verified_blocks.last()
        );
        self.events_state = events;
        Ok(())
    }

    fn get_verified_committed_blocks_transactions_from_blocks_state(
        &mut self,
        verified_blocks: &[EventData],
    ) -> Vec<FranklinTransaction> {
        info!("Loading new verified transactions");
        let committed_blocks = self
            .events_state
            .get_only_verified_committed_blocks(verified_blocks);
        // debug!("Committed verified blocks: {:?}", committed_blocks);
        let mut transactions = vec![];
        for block in committed_blocks {
            let tx = FranklinTransaction::get_transaction(&self.events_state.config, &block);
            if tx.is_none() {
                continue;
            }
            transactions.push(tx.expect("No franklin transaction in get_verified_committed_blocks_transactions_from_blocks_state"));
        }
        info!("Transactions loaded and sorted");
        transactions
    }

    fn sort_transactions_by_block_number(
        transactions: Vec<FranklinTransaction>,
    ) -> Vec<FranklinTransaction> {
        let mut sorted_transactions = transactions;
        sorted_transactions.sort_by_key(|x| x.block_number);
        debug!("Transactions sorted: by number");
        sorted_transactions
    }

    pub fn update_accounts_state_from_transactions(
        &mut self,
        transactions: &[FranklinTransaction],
    ) -> Result<(), DataRestoreError> {
        // let mut state = accounts_state::FranklinAccountsStates::new(config);
        info!("Start accounts state updating");
        for transaction in transactions {
            self.account_states
                .update_accounts_states_from_transaction(&transaction)
                .map_err(|e| DataRestoreError::StateUpdate(e.to_string()))?;
        }
        info!("Finished accounts state updating");

        let root = self.account_states.root_hash();
        info!("Root: {:?}", &root);
        info!(
            "Saving root, accounts list and transactions into file {}",
            FILENAME
        );
        {
            let f = File::create(FILENAME).expect("Unable to create file");
            let mut f = BufWriter::new(f);
            f.write(
                format!(
                    "Root hash on Franklin block {}: {}\n\nAccounts list: {:?}\n\nTransactions: {:?}",
                    transactions[transactions.len()-1].block_number,
                    root.to_hex(),
                    self.account_states.plasma_state.get_accounts(),
                    transactions
                )
                .as_bytes(),
            )
            .expect("Unable to write new root");
        }
        info!("Root saved in file");
        Ok(())
    }

    fn update_franklin_blocks_events_and_accounts_tree_state(
        &mut self,
    ) -> Result<(), DataRestoreError> {
        let mut new_events: (Vec<EventData>, Vec<EventData>) = (vec![], vec![]);
        while self.run_updates {
            info!("Loading new events");
            let ne = self
                .events_state
                .update_state_from_last_watched_block_with_blocks_delta_and_return_new_blocks(
                    self.blocks_delta,
                );
            match ne {
                Ok(result) => new_events = result,
                Err(error) => {
                    info!("Got no events: {:?}", error);
                    continue;
                }
            }
            if new_events.1.is_empty() {
                info!("No new verified blocks");
                continue;
            // return Err(DataRestoreError::NoData("No verified blocks".to_string()))
            } else {
                info!(
                    "Got new events state till ethereum block: {:?}",
                    &self.events_state.last_watched_block_number
                );
                info!(
                    "Committed franklin blocks count: {:?}",
                    &self.events_state.committed_blocks.len()
                );
                debug!(
                    "Last committed franklin block: {:?}",
                    &self.events_state.committed_blocks.last()
                );
                info!(
                    "Verified franklin blocks count: {:?}",
                    &self.events_state.verified_blocks.len()
                );
                debug!(
                    "Last verified franklin block: {:?}",
                    &self.events_state.verified_blocks.last()
                );
                break;
            }
        }
        if !self.run_updates {
            return Err(DataRestoreError::StateUpdate(
                "Stopped getting new blocks".to_string(),
            ));
        }
        let verified_blocks = &new_events.1;
        let txs =
            self.get_verified_committed_blocks_transactions_from_blocks_state(&verified_blocks);
        let sorted_txs = DataRestoreDriver::sort_transactions_by_block_number(txs);
        self.transactions.append(&mut sorted_txs.clone());

        self.update_accounts_state_from_transactions(&sorted_txs)
            .map_err(|e| DataRestoreError::StateUpdate(e.to_string()))?;

        // TODO: - shouldnt be here
        let mut logs = new_events.0.clone();
        logs.append(&mut new_events.1.clone());

        self.update_storage_state(&logs, &sorted_txs);

        Ok(())
    }
}
