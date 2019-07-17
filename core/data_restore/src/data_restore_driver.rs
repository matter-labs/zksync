use crate::accounts_state::FranklinAccountsStates;
use crate::block_events::BlockEventsFranklin;
use crate::blocks::LogBlockData;
use crate::franklin_transaction::FranklinTransaction;
use crate::helpers::*;
use models::plasma::Fr;
use std::sync::mpsc::Sender;
use web3::types::U256;

#[allow(dead_code)]
pub struct ProtoAccountsState {
    errored: bool,
    root_hash: Fr,
}

pub struct DataRestoreDriver {
    pub channel: Option<Sender<ProtoAccountsState>>,
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
        channel: Option<Sender<ProtoAccountsState>>,
    ) -> Self {
        Self {
            channel,
            config: config.clone(),
            genesis_block,
            blocks_delta,
            run_updates: false,
            block_events: BlockEventsFranklin::new(config.clone()),
            account_states: FranklinAccountsStates::new(config.clone()),
        }
    }

    pub fn load_past_state(&mut self) -> Result<(), DataRestoreError> {
        println!("Loading past state");
        let states = DataRestoreDriver::get_past_franklin_blocks_events_and_accounts_tree_state(
            self.config.clone(),
            self.genesis_block,
            self.blocks_delta,
        )
        .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        self.block_events = states.0;
        self.account_states = states.1;

        // let accs = &self.account_states.get_accounts();
        // println!("Accs: {:?}", accs);
        let root = self.account_states.root_hash();
        println!("Root: {:?}", &root);
        println!("______________");

        if let Some(ref _channel) = self.channel {
            let state = ProtoAccountsState {
                errored: false,
                root_hash: root,
            };
            let _send_result = _channel.send(state);
            if _send_result.is_err() {
                return Err(DataRestoreError::StateUpdate(
                    "Cant send last state".to_string(),
                ));
            }
        }
        println!("Finished loading past state");
        Ok(())
    }

    pub fn stop_state_updates(&mut self) {
        self.run_updates = false
    }

    pub fn run_state_updates(&mut self) -> Option<DataRestoreError> {
        println!("Start state updates");
        self.run_updates = true;
        let mut err: Option<DataRestoreError> = None;
        while self.run_updates {
            // match DataRestoreDriver::update_franklin_blocks_events_and_accounts_tree_state(self) {
            //     Err(error) => {
            //         println!("Something goes wrong: {:?}", error);
            //         self.run_updates = false;
            //         err = Some(DataRestoreError::StateUpdate(format!(
            //             "Error occured: {:?}",
            //             error
            //         )));
            //     }
            //     Ok(()) => {
            //         // println!("Updated, last watched ethereum block: {:?}", &self.block_events.last_watched_block_number);
            //         // println!("Committed franklin blocks count: {:?}", &self.block_events.committed_blocks.len());
            //         // println!("Last committed franklin block: {:?}", &self.block_events.committed_blocks.last());
            //         // println!("Verified franklin blocks count: {:?}", &self.block_events.verified_blocks.len());
            //         // println!("Last verified franklin block: {:?}", &self.block_events.verified_blocks.last());
            //         // let accs = self.account_states.get_accounts();
            //         // let root = self.account_states.root_hash();
            //         // println!("Accs: {:?}", accs);
            //         // println!("Root: {:?}", &root);
            //     }
            // };
            if let Err(error) =
                DataRestoreDriver::update_franklin_blocks_events_and_accounts_tree_state(self)
            {
                println!("Something goes wrong: {:?}", error);
                self.run_updates = false;
                err = Some(DataRestoreError::StateUpdate(format!(
                    "Error occured: {:?}",
                    error
                )));
            }
            let root = self.account_states.root_hash();
            println!("New root: {:?}", root);
            println!("______________");
            if let Some(ref _channel) = self.channel {
                let state = ProtoAccountsState {
                    errored: !self.run_updates,
                    root_hash: root,
                };
                let _send_result = _channel.send(state);
                if _send_result.is_err() {
                    self.run_updates = false;
                    err = Some(DataRestoreError::StateUpdate(
                        "Cant send last state".to_string(),
                    ));
                }
            }
        }
        println!("Stopped state updates");
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
        // println!("Last watched block: {:?}", events_state.last_watched_block_number);
        let verified_blocks = events_state.verified_blocks.clone();
        let txs = DataRestoreDriver::get_verified_committed_blocks_transactions_from_blocks_state(
            &events_state,
            &verified_blocks,
        );
        let sorted_txs = DataRestoreDriver::sort_transactions_by_block_number(txs);
        // println!("Transactions: {:?}", sorted_txs);

        let mut accounts_state = FranklinAccountsStates::new(config.clone());
        DataRestoreDriver::update_accounts_state_from_transactions(
            &mut accounts_state,
            &sorted_txs,
        )
        .map_err(|e| DataRestoreError::StateUpdate(e.to_string()))?;
        println!("Accounts and events state finished update");
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
        println!(
            "Got past events state till ethereum block: {:?}",
            &events.last_watched_block_number
        );
        println!(
            "Committed franklin blocks count: {:?}",
            &events.committed_blocks.len()
        );
        println!(
            "Last committed franklin block: {:?}",
            &events.committed_blocks.last()
        );
        println!(
            "Verified franklin blocks count: {:?}",
            &events.verified_blocks.len()
        );
        println!(
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
        // println!("Committed verified blocks: {:?}", committed_blocks);
        let mut transactions = vec![];
        for block in committed_blocks {
            let tx = FranklinTransaction::get_transaction(&block_events_state.config, &block);
            if tx.is_none() {
                continue;
            }
            transactions.push(tx.unwrap());
        }
        println!("Transactions sorted: only verified commited");
        transactions
    }

    fn sort_transactions_by_block_number(
        transactions: Vec<FranklinTransaction>,
    ) -> Vec<FranklinTransaction> {
        let mut sorted_transactions = transactions;
        sorted_transactions.sort_by_key(|x| x.block_number);
        println!("Transactions sorted: by number");
        sorted_transactions
    }

    fn update_accounts_state_from_transactions(
        state: &mut FranklinAccountsStates,
        transactions: &[FranklinTransaction],
    ) -> Result<(), DataRestoreError> {
        // let mut state = accounts_state::FranklinAccountsStates::new(config);
        println!("Start accounts state updating");
        for transaction in transactions {
            state
                .update_accounts_states_from_transaction(&transaction)
                .map_err(|e| DataRestoreError::StateUpdate(e.to_string()))?;
        }
        println!("Finished accounts state updating");
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
                    println!("Got no events: {:?}", error);
                    continue;
                }
            }
            if new_events.1.is_empty() {
                println!("No new verified blocks");
                continue;
            // return Err(DataRestoreError::NoData("No verified blocks".to_string()))
            } else {
                println!(
                    "Got new events state till ethereum block: {:?}",
                    &data_restore_driver.block_events.last_watched_block_number
                );
                println!(
                    "Committed franklin blocks count: {:?}",
                    &data_restore_driver.block_events.committed_blocks.len()
                );
                println!(
                    "Last committed franklin block: {:?}",
                    &data_restore_driver.block_events.committed_blocks.last()
                );
                println!(
                    "Verified franklin blocks count: {:?}",
                    &data_restore_driver.block_events.verified_blocks.len()
                );
                println!(
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
