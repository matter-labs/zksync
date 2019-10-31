use crate::accounts_state::FranklinAccountsState;
use crate::events_state::EventsState;
use crate::franklin_ops::FranklinOpsBlock;
use crate::genesis_state::get_genesis_state;
use crate::helpers::DataRestoreError;
use crate::storage_interactor;
use storage::ConnectionPool;

/// Storage state update
pub enum StorageUpdateState {
    None,
    Events,
    Operations,
}

/// Description of data restore driver
pub struct DataRestoreDriver {
    /// Database connection pool
    pub connection_pool: ConnectionPool,
    /// Step of the considered blocks ethereum block
    pub eth_blocks_delta: u64,
    /// Delta between last ethereum block and last watched ethereum block
    pub end_eth_blocks_delta: u64,
    /// Flag that indicates that state updates are running
    pub run_updates: bool,
    /// Franklin contract events state
    pub events_state: EventsState,
    /// Franklin accounts state
    pub accounts_state: FranklinAccountsState,
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
        eth_blocks_delta: u64,
        end_eth_blocks_delta: u64,
    ) -> Self {
        Self {
            connection_pool,
            eth_blocks_delta,
            end_eth_blocks_delta,
            run_updates: false,
            events_state: EventsState::new(),
            accounts_state: FranklinAccountsState::new(),
        }
    }

    /// Stop states updates by setting run_updates flag to false
    pub fn stop_state_updates(&mut self) {
        self.run_updates = false
    }

    pub fn load_state_from_storage(&mut self) -> Result<(), DataRestoreError> {
        let state = storage_interactor::get_storage_state(self.connection_pool.clone());
        if let Ok(state) = state {
            let tree_state = storage_interactor::get_tree_state(self.connection_pool.clone())?;
            self.accounts_state = FranklinAccountsState::load(tree_state.0, tree_state.1);
            match state {
                StorageUpdateState::Events => {
                    self.events_state = storage_interactor::get_events_state_from_storage(
                        self.connection_pool.clone(),
                    )?;
                    // Update operations
                    let new_ops_blocks = self.update_operations_state()?;
                    // Update tree
                    self.update_tree_state(new_ops_blocks)?;
                }
                StorageUpdateState::Operations => {
                    self.events_state = storage_interactor::get_events_state_from_storage(
                        self.connection_pool.clone(),
                    )?;
                    // Update operations
                    let new_ops_blocks = storage_interactor::get_ops_blocks_from_storage(
                        self.connection_pool.clone(),
                    )?;
                    // Update tree
                    self.update_tree_state(new_ops_blocks)?;
                }
                StorageUpdateState::None => {}
            }
            Ok(())
        } else {
            // If state is unknown then its empty or broken - start from beginning
            let genesis_acc_map = get_genesis_state()?;
            self.accounts_state = FranklinAccountsState::load(genesis_acc_map.0, genesis_acc_map.1);
            Ok(())
        }
    }

    pub fn run_state_updates(&mut self) -> Result<(), DataRestoreError> {
        self.run_updates = true;
        while self.run_updates {
            info!(
                "Last watched ethereum block: {:?}",
                &self.events_state.last_watched_eth_block_number
            );
            info!(
                "Committed franklin events count: {:?}",
                &self.events_state.committed_events.len()
            );
            info!(
                "Verified franklin events count: {:?}",
                &self.events_state.verified_events.len()
            );

            // Update events
            self.update_events_state()?;

            // Update operations
            let new_ops_blocks = self.update_operations_state()?;

            // Update tree
            self.update_tree_state(new_ops_blocks)?;
        }
        info!("Stopped state updates");
        Ok(())
    }

    fn update_events_state(&mut self) -> Result<(), DataRestoreError> {
        let events = self
            .events_state
            .update_events_state(self.eth_blocks_delta, self.end_eth_blocks_delta)?;
        info!("Got new events");

        // Store events
        storage_interactor::remove_events_state(self.connection_pool.clone())?;
        storage_interactor::save_events_state(&events, self.connection_pool.clone())?;

        storage_interactor::remove_storage_state_status(self.connection_pool.clone())?;
        storage_interactor::save_storage_state(
            StorageUpdateState::Events,
            self.connection_pool.clone(),
        )?;

        info!("Updated events storage");

        Ok(())
    }

    fn update_tree_state(
        &mut self,
        new_ops_blocks: Vec<FranklinOpsBlock>,
    ) -> Result<(), DataRestoreError> {
        for block in new_ops_blocks {
            let state = self
                .accounts_state
                .update_accounts_states_from_ops_block(&block)?;
            storage_interactor::update_tree_state(
                block.block_num,
                &state,
                self.connection_pool.clone(),
            )?;
        }

        storage_interactor::remove_storage_state_status(self.connection_pool.clone())?;
        storage_interactor::save_storage_state(
            StorageUpdateState::None,
            self.connection_pool.clone(),
        )?;

        info!("Updated accounts state");

        Ok(())
    }

    fn update_operations_state(&mut self) -> Result<Vec<FranklinOpsBlock>, DataRestoreError> {
        let new_blocks = self.get_new_operation_blocks_from_events()?;
        info!("Parsed events to operation blocks");

        storage_interactor::remove_franklin_ops(self.connection_pool.clone())?;
        storage_interactor::save_franklin_ops_blocks(&new_blocks, self.connection_pool.clone())?;

        storage_interactor::remove_storage_state_status(self.connection_pool.clone())?;
        storage_interactor::save_storage_state(
            StorageUpdateState::Operations,
            self.connection_pool.clone(),
        )?;

        info!("Updated events storage");

        Ok(new_blocks)
    }

    /// Return verified comitted operations blocks from verified op blocks events
    pub fn get_new_operation_blocks_from_events(
        &mut self,
    ) -> Result<Vec<FranklinOpsBlock>, DataRestoreError> {
        info!("Loading new verified op_blocks");
        let committed_events = self.events_state.get_only_verified_committed_events();
        let mut blocks: Vec<FranklinOpsBlock> = vec![];
        for event in committed_events {
            let mut _block = FranklinOpsBlock::get_from_event(&event)?;
            blocks.push(_block);
        }
        Ok(blocks)
    }
}
