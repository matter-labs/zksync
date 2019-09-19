use crate::accounts_state::FranklinAccountsStates;
use crate::events::EventData;
use crate::events_state::EventsState;
use crate::franklin_op_block::FranklinOpBlock;
use crate::helpers::*;
use crate::storage_interactor;
use storage::ConnectionPool;
use web3::types::U256;

/// Description of data restore driver
pub struct DataRestoreDriver {
    /// Database connection pool
    pub connection_pool: ConnectionPool,
    /// Step of the considered blocks ethereum block
    pub eth_blocks_delta: U256,
    /// Delta between last ethereum block and last watched ethereum block
    pub end_eth_blocks_delta: U256,
    /// Flag that indicates that state updates are running
    pub run_updates: bool,
    /// Franklin contract events state
    pub events_state: EventsState,
    /// Franklin accounts state
    pub account_states: FranklinAccountsStates,
    /// Franklin operations blocks
    pub op_blocks: Vec<FranklinOpBlock>,
}

impl DataRestoreDriver {
    /// Create new data restore driver
    ///
    /// # Arguments
    ///
    /// * `connection_pool` - Database connection pool
    /// * `eth_blocks_delta` - Step of the considered blocks ethereum block
    /// * `eth_end_blocks_delta` - Delta between last ethereum block and last watched ethereum block
    ///
    pub fn new(
        connection_pool: ConnectionPool,
        eth_blocks_delta: U256
        end_eth_blocks_delta: U256
    ) -> Self {
        Self {
            connection_pool,
            eth_blocks_delta,
            end_eth_blocks_delta,
            run_updates: false,
            events_state: EventsState::new(),
            account_states: FranklinAccountsStates::new(),
            op_blocks: vec![],
        }
    }

    /// Stop states updates by setting run_updates flag to false
    pub fn stop_state_updates(&mut self) {
        self.run_updates = false
    }

    pub fn run_state_updates(&mut self) -> Result<(), DataRestoreError> {
        self.run_updates = true
        while self.run_updates {
            info!(
                "Last watched ethereum block: {:?}",
                &self.events_state.last_watched_eth_block_number
            );
            info!(
                "Committed franklin blocks count: {:?}",
                &self.events_state.committed_blocks.len()
            );
            info!(
                "Verified franklin blocks count: {:?}",
                &self.events_state.verified_blocks.len()
            );

            let new_events = self.events_state.update_events_state(
                self.eth_blocks_delta.clone(),
                self.end_eth_blocks_delta.clone()
            )?;
            info!(
                "Got new events"
            );

            storage_interactor.update_events_list(&new_events)?;
            info!(
                "Updated events storage"
            );
            
            let new_operations = get_operations_from_events(&new_events)?;
            info!(
                "Parsed events to operations"
            );

            storage_interactor.update_operations_list(&new_operations)?;
            info!(
                "Updated operations storage"
            );

            self.account_states.update_accounts_state(&new_operations)?;
            info!(
                "Updated accounts state"
            );
        }
        info!("Stopped state updates");
        Ok(())
    }

    // /// Update past events and accounts states
    // ///
    // /// # Arguments
    // ///
    // /// * `until_block` - if some than it will update accounts states until specified Franklin block number
    // ///
    // fn update_past_franklin_blocks_events_and_accounts_tree_state(
    //     &mut self,
    //     until_block: Option<u32>,
    // ) -> Result<(), DataRestoreError> {
    //     let mut got_events = false;
    //     while !got_events {
    //         if let Ok(()) = self.update_past_blocks_events_state() {
    //             got_events = true;
    //         }
    //     }
    //     let verified_blocks = self.events_state.verified_blocks.clone();

    //     let op_blocks = self.get_verified_committed_op_blocks_from_blocks_state(&verified_blocks);
    //     let mut sorted_op_blocks = DataRestoreDriver::sort_op_blocks_by_block_number(op_blocks);

    //     self.op_blocks.append(&mut sorted_op_blocks.clone());

    //     self.account_states = FranklinAccountsStates::new(self.config.clone());

    //     if let Some(block) = until_block {
    //         sorted_op_blocks.retain(|x| x.block_number <= block);
    //     }
    //     self.update_accounts_state_from_op_blocks(&sorted_op_blocks)
    //         .map_err(|e| DataRestoreError::StateUpdate(e.to_string()))?;

    //     Ok(())
    // }

    // /// Load past events and accounts states
    // ///
    // /// # Arguments
    // ///
    // /// * `until_block` - if some than it will return accounts states until specified Franklin block number
    // ///
    // pub fn load_past_state(&mut self, until_block: Option<u32>) -> Result<(), DataRestoreError> {
    //     info!("Loading past state");
    //     self.update_past_franklin_blocks_events_and_accounts_tree_state(until_block)
    //         .map_err(|e| DataRestoreError::NoData(e.to_string()))?;

    //     self.save_complete_storage_state();

    //     info!("Finished loading past state");
    //     Ok(())
    // }

    // /// Stop states updates by setting run_updates flag to false
    // pub fn stop_state_updates(&mut self) {
    //     self.run_updates = false
    // }

    // /// Run updating events and accounts states. May produce error
    // pub fn run_state_updates(&mut self) -> Option<DataRestoreError> {
    //     info!("Start state updates");
    //     self.run_updates = true;
    //     let mut err: Option<DataRestoreError> = None;
    //     while self.run_updates {
    //         info!(
    //             "Last watched ethereum block: {:?}",
    //             &self.events_state.last_watched_block_number
    //         );
    //         info!(
    //             "Committed franklin blocks count: {:?}",
    //             &self.events_state.committed_blocks.len()
    //         );
    //         info!(
    //             "Verified franklin blocks count: {:?}",
    //             &self.events_state.verified_blocks.len()
    //         );
    //         if let Err(error) = self.update_franklin_blocks_events_and_accounts_tree_state() {
    //             error!("Something goes wrong: {:?}", error);
    //             self.run_updates = false;
    //             err = Some(DataRestoreError::StateUpdate(format!(
    //                 "Error occured: {:?}",
    //                 error
    //             )));
    //         }
    //     }
    //     info!("Stopped state updates");
    //     err
    // }

    

    // /// Save complete storage state: events, last watched ethereum block number, franklin operations blocks
    // fn save_complete_storage_state(&mut self) {
    //     info!("Saving storage state");
    //     let mut logs = self.events_state.committed_blocks.clone();
    //     logs.append(&mut self.events_state.verified_blocks.clone());

    //     storage_interactor::save_events_state(&logs, self.connection_pool.clone());
    //     storage_interactor::save_last_watched_block_number(
    //         &mut self.events_state.last_watched_block_number,
    //         self.connection_pool.clone(),
    //     );
    //     storage_interactor::save_franklin_op_blocks(&self.op_blocks, self.connection_pool.clone());
    //     info!("Storage state saved");
    // }

    // /// Update storage state
    // ///
    // /// # Arguments
    // ///
    // /// * `logs` - Franklin contract events
    // /// * `blocks` - Franklin operations blocks
    // ///
    // fn update_storage_state(&mut self, logs: &Vec<EventData>, blocks: &Vec<FranklinOpBlock>) {
    //     info!("Updating storage state");
    //     storage_interactor::save_events_state(&logs, self.connection_pool.clone());
    //     storage_interactor::save_last_watched_block_number(
    //         &self.events_state.last_watched_block_number,
    //         self.connection_pool.clone(),
    //     );
    //     storage_interactor::save_franklin_op_blocks(blocks, self.connection_pool.clone());
    //     info!("Storage state updated");
    // }

    // /// Update blocks events state
    // fn update_past_blocks_events_state(&mut self) -> Result<(), DataRestoreError> {
    //     info!("Loading events");
    //     let events = EventsState::get_past_state_from_genesis_with_blocks_delta(
    //         self.config.clone(),
    //         self.genesis_block.clone(),
    //         self.blocks_delta.clone(),
    //     )
    //     .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
    //     info!(
    //         "Got past events state till ethereum block: {:?}",
    //         &events.last_watched_block_number
    //     );
    //     info!(
    //         "Committed franklin blocks count: {:?}",
    //         &events.committed_blocks.len()
    //     );
    //     debug!(
    //         "Last committed franklin block: {:?}",
    //         &events.committed_blocks.last()
    //     );
    //     info!(
    //         "Verified franklin blocks count: {:?}",
    //         &events.verified_blocks.len()
    //     );
    //     debug!(
    //         "Last verified franklin block: {:?}",
    //         &events.verified_blocks.last()
    //     );
    //     self.events_state = events;
    //     Ok(())
    // }

    // /// Return verified comitted operations blocks from verified op blocks events
    // ///
    // /// # Arguments
    // ///
    // /// * `verified_blocks` - Franklin verified op blocks events
    // ///
    // fn get_verified_committed_op_blocks_from_blocks_state(
    //     &mut self,
    //     verified_blocks: &[EventData],
    // ) -> Vec<FranklinOpBlock> {
    //     info!("Loading new verified op_blocks");
    //     let committed_blocks = self
    //         .events_state
    //         .get_only_verified_committed_blocks(verified_blocks);
    //     let mut op_blocks = vec![];
    //     for block in committed_blocks {
    //         let tx = FranklinOpBlock::get_franklin_op_block(&self.events_state.config, &block);
    //         if tx.is_none() {
    //             continue;
    //         }
    //         op_blocks.push(tx.expect(
    //             "No franklin op_blocks in get_verified_committed_op_blocks_from_blocks_state",
    //         ));
    //     }
    //     info!("Operation blocks loaded and sorted");
    //     op_blocks
    // }

    // /// Return operations blocks sorted by number
    // ///
    // /// # Arguments
    // ///
    // /// * `op_blocks` - Franklin operations blocks
    // ///
    // fn sort_op_blocks_by_block_number(op_blocks: Vec<FranklinOpBlock>) -> Vec<FranklinOpBlock> {
    //     let mut sorted_op_blocks = op_blocks;
    //     sorted_op_blocks.sort_by_key(|x| x.block_number);
    //     debug!("Op blocks sorted: by number");
    //     sorted_op_blocks
    // }

    // /// Update accounts state from operations blocks
    // ///
    // /// # Arguments
    // ///
    // /// * `op_blocks` - Franklin operations blocks
    // ///
    // pub fn update_accounts_state_from_op_blocks(
    //     &mut self,
    //     op_blocks: &[FranklinOpBlock],
    // ) -> Result<(), DataRestoreError> {
    //     info!("Start accounts state updating");
    //     for op_block in op_blocks {
    //         self.account_states
    //             .update_accounts_states_from_op_block(&op_block)
    //             .map_err(|e| DataRestoreError::StateUpdate(e.to_string()))?;
    //     }
    //     info!("Finished accounts state updating");

    //     let root = self.account_states.root_hash();
    //     info!("Root: {:?}", &root);
    //     Ok(())
    // }

    // /// Update events and accounts states
    // fn update_franklin_blocks_events_and_accounts_tree_state(
    //     &mut self,
    // ) -> Result<(), DataRestoreError> {
    //     let mut new_events: (Vec<EventData>, Vec<EventData>) = (vec![], vec![]);
    //     while self.run_updates {
    //         info!("Loading new events");
    //         let ne = self
    //             .events_state
    //             .update_state_from_last_watched_block_with_blocks_delta_and_return_new_blocks(
    //                 self.blocks_delta,
    //             );
    //         match ne {
    //             Ok(result) => new_events = result,
    //             Err(error) => {
    //                 info!("Got no events: {:?}", error);
    //                 continue;
    //             }
    //         }
    //         if new_events.1.is_empty() {
    //             info!("No new verified blocks");
    //             continue;
    //         } else {
    //             info!(
    //                 "Got new events state till ethereum block: {:?}",
    //                 &self.events_state.last_watched_block_number
    //             );
    //             info!(
    //                 "Committed franklin blocks count: {:?}",
    //                 &self.events_state.committed_blocks.len()
    //             );
    //             debug!(
    //                 "Last committed franklin block: {:?}",
    //                 &self.events_state.committed_blocks.last()
    //             );
    //             info!(
    //                 "Verified franklin blocks count: {:?}",
    //                 &self.events_state.verified_blocks.len()
    //             );
    //             debug!(
    //                 "Last verified franklin block: {:?}",
    //                 &self.events_state.verified_blocks.last()
    //             );
    //             break;
    //         }
    //     }
    //     if !self.run_updates {
    //         return Err(DataRestoreError::StateUpdate(
    //             "Stopped getting new blocks".to_string(),
    //         ));
    //     }
    //     let verified_blocks = &new_events.1;
    //     let op_blocks = self.get_verified_committed_op_blocks_from_blocks_state(&verified_blocks);
    //     let sorted_op_blocks = DataRestoreDriver::sort_op_blocks_by_block_number(op_blocks);
    //     self.op_blocks.append(&mut sorted_op_blocks.clone());

    //     self.update_accounts_state_from_op_blocks(&sorted_op_blocks)
    //         .map_err(|e| DataRestoreError::StateUpdate(e.to_string()))?;

    //     let mut logs = new_events.0.clone();
    //     logs.append(&mut new_events.1.clone());

    //     self.update_storage_state(&logs, &sorted_op_blocks);

    //     Ok(())
    // }
}
