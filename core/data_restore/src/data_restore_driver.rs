use crate::tree_state::TreeState;
use crate::events_state::EventsState;
use ethabi;
use crate::rollup_ops::RollupOpsBlock;
use crate::genesis_state::get_genesis_state;
use crate::storage_interactor;
use storage::ConnectionPool;
use models::node::account::AccountAddress;
use models::node::{AccountMap, AccountUpdate};
use web3::contract::Contract;
use web3::types::H160;
use web3::{Transport, Web3};
use web3::types::H256;
use crate::helpers::get_ethereum_transaction;
use std::str::FromStr;

/// Storage state update
pub enum StorageUpdateState {
    None,
    Events,
    Operations,
}

/// Description of data restore driver
pub struct DataRestoreDriver<T: Transport> {
    /// Database connection pool
    pub connection_pool: ConnectionPool,
    /// Web3 endpoint
    pub web3: Web3<T>,
    /// Provides Ethereum Franklin contract unterface
    pub franklin_contract: (ethabi::Contract, Contract<T>),
    /// Flag that indicates that state updates are running
    pub run_update: bool,
    /// Franklin contract events state
    pub events_state: EventsState,
    /// Franklin accounts state
    pub tree_state: TreeState,
    pub eth_blocks_step: u64,
    pub end_eth_blocks_offset: u64
}

impl<T: Transport> DataRestoreDriver<T> {
    pub fn new_empty(
        connection_pool: ConnectionPool,
        web3: Web3<T>,
        contract_eth_addr: H160,
        eth_blocks_step: u64,
        end_eth_blocks_offset: u64
    ) -> Result<Self, failure::Error> {

        storage_interactor::remove_events_state(connection_pool.clone())?;
        storage_interactor::remove_rollup_ops(connection_pool.clone())?;
        storage_interactor::remove_tree_state(connection_pool.clone())?;

        let franklin_contract = {
            let abi_string = serde_json::Value::from_str(models::abi::FRANKLIN_CONTRACT)
                .unwrap()
                .get("abi")
                .unwrap()
                .to_string();
            let abi = ethabi::Contract::load(abi_string.as_bytes()).unwrap();
            (
                abi.clone(),
                Contract::new(web3.eth(), contract_eth_addr, abi.clone()),
            )
        };

        let events_state = EventsState::new();
        
        let tree_state = TreeState::new();

        Ok(Self {
            connection_pool,
            web3,
            franklin_contract,
            run_update: false,
            events_state,
            tree_state,
            eth_blocks_step,
            end_eth_blocks_offset
        })
    }

    /// Create new data restore driver
    ///
    /// # Arguments
    ///
    /// * `connection_pool` - Database connection pool
    /// * `eth_blocks_step` - Step of the considered blocks ethereum block
    /// * `eth_end_blocks_delta` - Delta between last ethereum block and last watched ethereum block
    ///
    pub fn new_from_genesis(
        connection_pool: ConnectionPool,
        web3: Web3<T>,
        contract_eth_addr: H160,
        contract_genesis_tx_hash: H256,
        eth_blocks_step: u64,
        end_eth_blocks_offset: u64,
    ) -> Result<Self, failure::Error> {

        storage_interactor::remove_events_state(connection_pool.clone())?;
        storage_interactor::remove_rollup_ops(connection_pool.clone())?;
        storage_interactor::remove_tree_state(connection_pool.clone())?;

        let franklin_contract = {
            let abi_string = serde_json::Value::from_str(models::abi::FRANKLIN_CONTRACT)
                .unwrap()
                .get("abi")
                .unwrap()
                .to_string();
            let abi = ethabi::Contract::load(abi_string.as_bytes()).unwrap();
            (
                abi.clone(),
                Contract::new(web3.eth(), contract_eth_addr, abi.clone()),
            )
        };

        let events_state = EventsState::new();

        let genesis_transaction = get_ethereum_transaction(&web3, &contract_genesis_tx_hash)?;
        let (block_number, genesis_account) = get_genesis_state(&genesis_transaction)?;
        
        let genesis_eth_block_number = events_state.set_genesis_block_number(&genesis_transaction)?;

        let account_update = AccountUpdate::Create {
            address: genesis_account.address.clone(),
            nonce: genesis_account.nonce.clone(),
        };

        let mut account_map = AccountMap::default();
        account_map.insert(block_number, genesis_account);
        
        let tree_state = TreeState::load(
            block_number,
            account_map,
            0,
            genesis_account.address.clone(),
        );

        storage_interactor::save_events_state(
            connection_pool.clone(),
            &vec![],
            genesis_eth_block_number
        )?;

        storage_interactor::save_tree_state(
            connection_pool.clone(),
            block_number,
            &account_map,
            0,
            genesis_account.address.clone(),
        )?;

        Ok(Self {
            connection_pool,
            web3,
            franklin_contract,
            run_update: false,
            events_state,
            tree_state,
            eth_blocks_step,
            end_eth_blocks_offset
        })
    }

    /// Stop states updates by setting run_update flag to false
    pub fn stop_state_update(&mut self) {
        self.run_update = false
    }

    pub fn load_state_from_storage(&mut self) -> Result<(), failure::Error> {
        let state = storage_interactor::get_storage_state(self.connection_pool.clone())?;
        let tree_state = storage_interactor::get_tree_state(self.connection_pool.clone())?;
        self.tree_state = TreeState::load(
            tree_state.0, // current bblock
            tree_state.1, // account map
            tree_state.2, // unprocessed priority op
            tree_state.3 // fee account
        );
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
    }

    pub fn run_state_update(&mut self) -> Result<(), failure::Error> {
        self.run_update = true;
        while self.run_update {
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

    fn update_events_state(&mut self) -> Result<(), failure::Error> {
        let (events, last_watched_eth_block_number) = self
            .events_state
            .update_events_state(&self.web3, &self.franklin_contract, self.eth_blocks_step, self.end_eth_blocks_offset)?;
        info!("Got new events");

        // Store events
        storage_interactor::remove_events_state(self.connection_pool.clone())?;
        storage_interactor::save_events_state(
            self.connection_pool.clone(),
            &events,
            last_watched_eth_block_number
        )?;

        storage_interactor::remove_storage_state_status(self.connection_pool.clone())?;
        storage_interactor::save_storage_state(
            self.connection_pool.clone(),
            StorageUpdateState::Events,
        )?;

        info!("Updated events storage");

        Ok(())
    }

    fn update_tree_state(
        &mut self,
        new_ops_blocks: Vec<RollupOpsBlock>,
    ) -> Result<(), failure::Error> {
        for block in new_ops_blocks {
            let commit = self
                .tree_state
                .update_tree_states_from_ops_block(&block)?;
            storage_interactor::update_tree_state(
                self.connection_pool.clone(),
                commit,
                self.tree_state.current_unprocessed_priority_op,
                self.tree_state.last_fee_account_address.clone()
            )?;
        }

        storage_interactor::remove_storage_state_status(self.connection_pool.clone())?;
        storage_interactor::save_storage_state(
            self.connection_pool.clone(),
            StorageUpdateState::None,
        )?;

        info!("Updated accounts state");

        Ok(())
    }

    fn update_operations_state(&mut self) -> Result<Vec<RollupOpsBlock>, failure::Error> {
        let new_blocks = self.get_new_operation_blocks_from_events()?;
        info!("Parsed events to operation blocks");

        storage_interactor::remove_rollup_ops(self.connection_pool.clone())?;
        storage_interactor::save_rollup_ops(self.connection_pool.clone(), &new_blocks)?;

        storage_interactor::remove_storage_state_status(self.connection_pool.clone())?;
        storage_interactor::save_storage_state(
            self.connection_pool.clone(),
            StorageUpdateState::Operations,
        )?;

        info!("Updated events storage");

        Ok(new_blocks)
    }

    /// Return verified comitted operations blocks from verified op blocks events
    pub fn get_new_operation_blocks_from_events(
        &mut self,
    ) -> Result<Vec<RollupOpsBlock>, failure::Error> {
        info!("Loading new verified op_blocks");
        let committed_events = self.events_state.get_only_verified_committed_events();
        let mut blocks: Vec<RollupOpsBlock> = vec![];
        for event in committed_events {
            let mut _block = RollupOpsBlock::get_rollup_ops_block(&self.web3, &event)?;
            blocks.push(_block);
        }
        Ok(blocks)
    }
}
