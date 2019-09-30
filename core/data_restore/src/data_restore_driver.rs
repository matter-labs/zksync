use crate::accounts_state::FranklinAccountsState;
use crate::events::EventData;
use crate::events_state::EventsState;
use models::node::operations::{
    TX_TYPE_BYTES_LEGTH, DepositOp, FranklinOp, FullExitOp, TransferOp, TransferToNewOp, WithdrawOp,
};
use crate::franklin_ops::FranklinOpsBlock;
use models::node::priority_ops::{Deposit, FranklinPriorityOp, FullExit};
use models::node::tx::{Close, FranklinTx, Transfer, Withdraw};
use models::node::account::{Account, AccountAddress, AccountUpdate};
use crate::helpers::*;
use crate::storage_interactor;
use crate::franklin_ops;
use storage::ConnectionPool;
// use web3::types::U256;

pub enum StorageUpdateState {
    None,
    Events,
    Operations
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
    /// Franklin operations blocks
    pub op_blocks: Vec<FranklinOpsBlock>,
    /// Storage update state
    pub storage_update_state: StorageUpdateState,
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
        genesis_block_number: u64,
        eth_blocks_delta: u64,
        end_eth_blocks_delta: u64
    ) -> Self {
        Self {
            connection_pool,
            eth_blocks_delta,
            end_eth_blocks_delta,
            run_updates: false,
            events_state: EventsState::new(genesis_block_number),
            accounts_state: FranklinAccountsState::new(),
            op_blocks: vec![],
            storage_update_state: StorageUpdateState::None
        }
    }

    /// Stop states updates by setting run_updates flag to false
    pub fn stop_state_updates(&mut self) {
        self.run_updates = false
    }

    pub fn load_state_from_storage(&mut self) -> Result<(), DataRestoreError> {
        // match self.storage_update_state {

        // }
        Ok(())
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
            let events = self.events_state.update_events_state(
                self.eth_blocks_delta.clone(),
                self.end_eth_blocks_delta.clone()
            )?;
            info!(
                "Got new events"
            );

            // Store events
            storage_interactor::remove_events_state(self.connection_pool.clone())?;
            storage_interactor::save_events_state(&events, self.connection_pool.clone())?;
            info!(
                "Updated events storage"
            );
            
            self.storage_update_state = StorageUpdateState::Events;
            
            // Update operations
            let new_blocks = self.get_new_operation_blocks_from_events()?;
            info!(
                "Parsed events to operation blocks"
            );

            storage_interactor::remove_franklin_ops(self.connection_pool.clone())?;
            storage_interactor::save_franklin_ops_blocks(&new_blocks, self.connection_pool.clone())?;
            info!(
                "Updated events storage"
            );

            self.storage_update_state = StorageUpdateState::Operations;

            // Update tree
            for block in new_blocks {
                let state = self.accounts_state.update_accounts_states_from_ops_block(&block)?;
                storage_interactor::update_tree_state(block.block_num, &state, self.connection_pool.clone())?;
            }
            info!(
                "Updated accounts state"
            );

            self.storage_update_state = StorageUpdateState::None;
        }
        info!("Stopped state updates");
        Ok(())
    }

    /// Return verified comitted operations blocks from verified op blocks events
    pub fn get_new_operation_blocks_from_events(&mut self) -> Result<Vec<FranklinOpsBlock>, DataRestoreError> {
        info!("Loading new verified op_blocks");
        let committed_events = self
            .events_state
            .get_only_verified_committed_events();
        let mut blocks: Vec<FranklinOpsBlock> = vec![];
        for event in committed_events {
            let mut _block = FranklinOpsBlock::get_from_event(&event)?;
            blocks.push(_block);
        }
        Ok(blocks)
    }
}
