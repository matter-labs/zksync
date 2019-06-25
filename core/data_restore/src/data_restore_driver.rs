use crate::accounts_state::FranklinAccountsStates;
use crate::block_events::BlockEventsFranklin;
use crate::blocks::LogBlockData;
use crate::franklin_transaction::FranklinTransaction;
use crate::helpers::*;
use std::fs::File;
use std::io::{BufWriter, Write};
use web3::types::U256;

const FILENAME: &str = "restored_data.txt";

pub struct DataRestoreDriver {
    pub config: DataRestoreConfig,
    pub genesis_block: U256,
    pub blocks_delta: U256,
    pub run_updates: bool,
    pub block_events: BlockEventsFranklin,
    pub account_states: FranklinAccountsStates,
}

impl DataRestoreDriver {
    pub fn new(
        config: DataRestoreConfig,
        genesis_block: U256,
        blocks_delta: U256,
    ) -> Self {
        Self {
            config: config.clone(),
            genesis_block,
            blocks_delta,
            run_updates: false,
            block_events: BlockEventsFranklin::new(config.clone()),
            account_states: FranklinAccountsStates::new(config.clone()),
        }
    }

    pub fn load_past_state(&mut self) -> Result<(), DataRestoreError> {
        info!("Loading past state");
        let states = DataRestoreDriver::get_past_franklin_blocks_events_and_accounts_tree_state(
            self.config.clone(),
            self.genesis_block,
            self.blocks_delta,
        )
        .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        self.block_events = states.0;
        self.account_states = states.1;

        // let accs = &self.account_states.get_accounts();
        // debug!("Accs: {:?}", accs);
        let root = self.account_states.root_hash();
        info!("Root: {:?}", &root);
        {
            let f = File::create(FILENAME).expect("Unable to create file");
            let mut f = BufWriter::new(f);
            f
                .write(format!("Root hash on Franklin block {} is {}\n\nAccounts list: {:?}", self.block_events.verified_blocks.len(), root.to_hex(), self.account_states.plasma_state.get_accounts()).as_bytes())
                .expect("Unable to write new root");
        }
        info!("Root saved in file");
        info!("Finished loading past state");
        Ok(())
    }

    pub fn stop_state_updates(&mut self) {
        self.run_updates = false
    }

    pub fn run_state_updates(&mut self) -> Option<DataRestoreError> {
        info!("Start state updates");
        self.run_updates = true;
        let mut err: Option<DataRestoreError> = None;
        while self.run_updates {
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
            //         // debug!("Updated, last watched ethereum block: {:?}", &self.block_events.last_watched_block_number);
            //         // debug!("Committed franklin blocks count: {:?}", &self.block_events.committed_blocks.len());
            //         // debug!("Last committed franklin block: {:?}", &self.block_events.committed_blocks.last());
            //         // debug!("Verified franklin blocks count: {:?}", &self.block_events.verified_blocks.len());
            //         // debug!("Last verified franklin block: {:?}", &self.block_events.verified_blocks.last());
            //         // let accs = self.account_states.get_accounts();
            //         // let root = self.account_states.root_hash();
            //         // debug!("Accs: {:?}", accs);
            //         // debug!("Root: {:?}", &root);
            //     }
            // };
            if let Err(error) =
                DataRestoreDriver::update_franklin_blocks_events_and_accounts_tree_state(self)
            {
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

    fn get_past_franklin_blocks_events_and_accounts_tree_state(
        config: DataRestoreConfig,
        genesis_block: U256,
        blocks_delta: U256,
    ) -> Result<(BlockEventsFranklin, FranklinAccountsStates), DataRestoreError> {
        let events_state =
            DataRestoreDriver::get_past_blocks_state(config.clone(), genesis_block, blocks_delta)
                .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        // debug!("Last watched block: {:?}", events_state.last_watched_block_number);
        let verified_blocks = events_state.verified_blocks.clone();
        let txs = DataRestoreDriver::get_verified_committed_blocks_transactions_from_blocks_state(
            &events_state,
            &verified_blocks,
        );
        let sorted_txs = DataRestoreDriver::sort_transactions_by_block_number(txs);
        // debug!("Transactions: {:?}", sorted_txs);

        let mut accounts_state = FranklinAccountsStates::new(config.clone());
        DataRestoreDriver::update_accounts_state_from_transactions(
            &mut accounts_state,
            &sorted_txs,
        )
        .map_err(|e| DataRestoreError::StateUpdate(e.to_string()))?;
        info!("Accounts and events state finished update");
        Ok((events_state, accounts_state))
    }

    fn get_past_blocks_state(
        config: DataRestoreConfig,
        genesis_block: U256,
        blocks_delta: U256,
    ) -> Result<BlockEventsFranklin, DataRestoreError> {
        let events = BlockEventsFranklin::get_past_state_from_genesis_with_blocks_delta(
            config,
            genesis_block,
            blocks_delta,
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
        Ok(events)
    }

    fn get_verified_committed_blocks_transactions_from_blocks_state(
        block_events_state: &BlockEventsFranklin,
        verified_blocks: &[LogBlockData],
    ) -> Vec<FranklinTransaction> {
        let committed_blocks =
            block_events_state.get_only_verified_committed_blocks(verified_blocks);
        // debug!("Committed verified blocks: {:?}", committed_blocks);
        let mut transactions = vec![];
        for block in committed_blocks {
            let tx = FranklinTransaction::get_transaction(&block_events_state.config, &block);
            if tx.is_none() {
                continue;
            }
            transactions.push(tx.unwrap());
        }
        debug!("Transactions sorted: only verified commited");
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

    fn update_accounts_state_from_transactions(
        state: &mut FranklinAccountsStates,
        transactions: &[FranklinTransaction],
    ) -> Result<(), DataRestoreError> {
        // let mut state = accounts_state::FranklinAccountsStates::new(config);
        debug!("Start accounts state updating");
        for transaction in transactions {
            state
                .update_accounts_states_from_transaction(&transaction)
                .map_err(|e| DataRestoreError::StateUpdate(e.to_string()))?;
        }
        debug!("Finished accounts state updating");
        Ok(())
    }

    fn update_franklin_blocks_events_and_accounts_tree_state(
        data_restore_driver: &mut DataRestoreDriver,
    ) -> Result<(), DataRestoreError> {
        let mut new_events: (Vec<LogBlockData>, Vec<LogBlockData>) = (vec![], vec![]);
        while data_restore_driver.run_updates {
            let ne = data_restore_driver
                .block_events
                .update_state_from_last_watched_block_with_blocks_delta_and_return_new_blocks(
                    data_restore_driver.blocks_delta,
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
                    &data_restore_driver.block_events.last_watched_block_number
                );
                info!(
                    "Committed franklin blocks count: {:?}",
                    &data_restore_driver.block_events.committed_blocks.len()
                );
                debug!(
                    "Last committed franklin block: {:?}",
                    &data_restore_driver.block_events.committed_blocks.last()
                );
                info!(
                    "Verified franklin blocks count: {:?}",
                    &data_restore_driver.block_events.verified_blocks.len()
                );
                debug!(
                    "Last verified franklin block: {:?}",
                    &data_restore_driver.block_events.verified_blocks.last()
                );
                break;
            }
        }
        if !data_restore_driver.run_updates {
            return Err(DataRestoreError::StateUpdate(
                "Stopped getting new blocks".to_string(),
            ));
        }
        let verified_blocks = &new_events.1;
        let txs = DataRestoreDriver::get_verified_committed_blocks_transactions_from_blocks_state(
            &data_restore_driver.block_events,
            &verified_blocks,
        );
        let sorted_txs = DataRestoreDriver::sort_transactions_by_block_number(txs);

        DataRestoreDriver::update_accounts_state_from_transactions(
            &mut data_restore_driver.account_states,
            &sorted_txs,
        )
        .map_err(|e| DataRestoreError::StateUpdate(e.to_string()))?;

        Ok(())
    }
}
