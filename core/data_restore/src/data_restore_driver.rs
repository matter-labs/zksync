use crate::contract_functions::{get_genesis_account, get_total_verified_blocks};
use crate::eth_tx_helpers::get_ethereum_transaction;
use crate::events_state::EventsState;
use crate::rollup_ops::RollupOpsBlock;
use crate::storage_interactor;
use crate::tree_state::TreeState;
use models::abi::{governance_contract, zksync_contract};
use models::node::{AccountMap, AccountUpdate};
use storage::ConnectionPool;
use web3::contract::Contract;
use web3::types::{H160, H256};
use web3::{Transport, Web3};

/// Storage state update:
/// - None - The state is updated completely last time - start from fetching the new events
/// - Events - The events fetched and saved successfully - now get operations from them and update tree
/// - Operations - There are operations that are not presented in the tree state - update tree state
#[derive(Debug)]
pub enum StorageUpdateState {
    None,
    Events,
    Operations,
}

/// Data restore driver is a high level interface for all restoring components.
/// It is actually a finite state machine, that has following states:
/// - Empty - The state is new
/// - None - The state is completely updated last time, driver will load state from storage and fetch new events
/// - Events - The events has been fetched and saved successfully and firstly driver will load state from storage and get new operation for last saved events
/// - Operations - The operations and events has been fetched and saved successfully and firstly driver will load state from storage and update merkle tree by last saved operations
/// Driver can interact with other restoring components for their updating:
/// - Events
/// - Operations
/// - Tree
/// - Storage
pub struct DataRestoreDriver<T: Transport> {
    /// Database connection pool
    pub connection_pool: ConnectionPool,
    /// Web3 provider endpoint
    pub web3: Web3<T>,
    /// Provides Ethereum Governance contract unterface
    pub governance_contract: (ethabi::Contract, Contract<T>),
    /// Provides Ethereum Rollup contract unterface
    pub franklin_contract: (ethabi::Contract, Contract<T>),
    /// Rollup contract events state
    pub events_state: EventsState,
    /// Rollup accounts state
    pub tree_state: TreeState,
    /// The step distance of viewing events in the ethereum blocks
    pub eth_blocks_step: u64,
    /// The distance to the last ethereum block
    pub end_eth_blocks_offset: u64,
}

impl<T: Transport> DataRestoreDriver<T> {
    /// Returns new data restore driver with empty events and tree states.
    ///
    /// # Arguments
    ///
    /// * `connection_pool` - Database connection pool
    /// * `web3_transport` - Web3 provider transport
    /// * `governance_contract_eth_addr` - Governance contract address
    /// * `franklin_contract_eth_addr` - Rollup contract address
    /// * `eth_blocks_step` - The step distance of viewing events in the ethereum blocks
    /// * `end_eth_blocks_offset` - The distance to the last ethereum block
    ///
    pub fn new(
        connection_pool: ConnectionPool,
        web3_transport: T,
        governance_contract_eth_addr: H160,
        franklin_contract_eth_addr: H160,
        eth_blocks_step: u64,
        end_eth_blocks_offset: u64,
    ) -> Self {
        let web3 = Web3::new(web3_transport);

        let governance_contract = {
            let abi = governance_contract();
            (
                abi.clone(),
                Contract::new(web3.eth(), governance_contract_eth_addr, abi.clone()),
            )
        };

        let franklin_contract = {
            let abi = zksync_contract();
            (
                abi.clone(),
                Contract::new(web3.eth(), franklin_contract_eth_addr, abi.clone()),
            )
        };

        let events_state = EventsState::default();

        let tree_state = TreeState::new();

        Self {
            connection_pool,
            web3,
            governance_contract,
            franklin_contract,
            events_state,
            tree_state,
            eth_blocks_step,
            end_eth_blocks_offset,
        }
    }

    /// Sets the 'genesis' state.
    /// Tree with inserted genesis account will be created.
    /// Used when restore driver is restarted.
    ///
    /// # Arguments
    ///
    /// * `governance_contract_genesis_tx_hash` - Governance contract creation tx hash
    /// * `franklin_contract_genesis_tx_hash` - Rollup contract creation tx hash
    ///
    pub fn set_genesis_state(
        &mut self,
        governance_contract_genesis_tx_hash: H256,
        franklin_contract_genesis_tx_hash: H256,
    ) {
        let genesis_franklin_transaction =
            get_ethereum_transaction(&self.web3, &franklin_contract_genesis_tx_hash)
                .expect("Cant get franklin genesis transaction");
        let genesis_governance_transaction =
            get_ethereum_transaction(&self.web3, &governance_contract_genesis_tx_hash)
                .expect("Cant get governance genesis transaction");

        // Setting genesis block number for events state
        let genesis_eth_block_number = self
            .events_state
            .set_genesis_block_number(&genesis_governance_transaction)
            .expect("Cant set genesis block number for events state");
        info!("genesis_eth_block_number: {:?}", &genesis_eth_block_number);

        storage_interactor::save_events_state(
            &self.connection_pool,
            &[],
            &[],
            genesis_eth_block_number,
        );

        let genesis_account = get_genesis_account(&genesis_franklin_transaction)
            .expect("Cant get genesis account address");

        let account_update = AccountUpdate::Create {
            address: genesis_account.address.clone(),
            nonce: genesis_account.nonce,
        };

        let mut account_map = AccountMap::default();
        account_map.insert(0, genesis_account.clone());

        let current_block = 0;
        let current_unprocessed_priority_op = 0;
        let fee_acc_num = 0;

        let tree_state = TreeState::load(
            current_block,
            account_map,
            current_unprocessed_priority_op,
            fee_acc_num,
        );

        info!("Genesis tree root hash: {:?}", tree_state.root_hash());
        debug!("Genesis accounts: {:?}", tree_state.get_accounts());

        storage_interactor::save_genesis_tree_state(&self.connection_pool, account_update);

        info!("Saved genesis tree state\n");

        self.tree_state = tree_state;
    }

    /// Stops states from storage
    pub fn load_state_from_storage(&mut self) {
        info!("Loading state from storage");
        let state = storage_interactor::get_storage_state(&self.connection_pool);
        self.events_state =
            storage_interactor::get_block_events_state_from_storage(&self.connection_pool);
        let tree_state = storage_interactor::get_tree_state(&self.connection_pool);
        self.tree_state = TreeState::load(
            tree_state.0, // current block
            tree_state.1, // account map
            tree_state.2, // unprocessed priority op
            tree_state.3, // fee account
        );
        match state {
            StorageUpdateState::Events => {
                // Update operations
                let new_ops_blocks = self.update_operations_state();
                // Update tree
                self.update_tree_state(new_ops_blocks);
            }
            StorageUpdateState::Operations => {
                // Update operations
                let new_ops_blocks =
                    storage_interactor::get_ops_blocks_from_storage(&self.connection_pool);
                // Update tree
                self.update_tree_state(new_ops_blocks);
            }
            StorageUpdateState::None => {}
        }
        let total_verified_blocks = get_total_verified_blocks(&self.franklin_contract);
        let last_verified_block = self.tree_state.state.block_number;
        info!(
            "State has been loaded\nProcessed {:?} blocks of total {:?} verified on contract\nRoot hash: {:?}\n",
            last_verified_block,
            total_verified_blocks,
            self.tree_state.root_hash()
        );
    }

    /// Activates states updates
    pub fn run_state_update(&mut self) {
        let mut last_wached_block: u64 = self.events_state.last_watched_eth_block_number;
        loop {
            debug!("Last watched ethereum block: {:?}", last_wached_block);

            // Update events
            if self.update_events_state() {
                // Update operations
                let new_ops_blocks = self.update_operations_state();

                if !new_ops_blocks.is_empty() {
                    // Update tree
                    self.update_tree_state(new_ops_blocks);

                    let total_verified_blocks = get_total_verified_blocks(&self.franklin_contract);
                    let last_verified_block = self.tree_state.state.block_number;
                    info!(
                        "State updated\nProcessed {:?} blocks of total {:?} verified on contract\nRoot hash: {:?}\n",
                        last_verified_block,
                        total_verified_blocks,
                        self.tree_state.root_hash()
                    );
                }
            }

            if last_wached_block == self.events_state.last_watched_eth_block_number {
                std::thread::sleep(std::time::Duration::from_secs(5));
            } else {
                last_wached_block = self.events_state.last_watched_eth_block_number;
            }
        }
    }

    /// Updates events state, saves new blocks, tokens events and the last watched eth block number in storage
    /// Returns bool flag, true if there are new block events
    fn update_events_state(&mut self) -> bool {
        let (block_events, token_events, last_watched_eth_block_number) = self
            .events_state
            .update_events_state(
                &self.web3,
                &self.franklin_contract,
                &self.governance_contract,
                self.eth_blocks_step,
                self.end_eth_blocks_offset,
            )
            .expect("Updating events state: cant update events state");

        storage_interactor::save_events_state(
            &self.connection_pool,
            &block_events,
            token_events.as_slice(),
            last_watched_eth_block_number,
        );

        debug!("Updated events storage");

        !block_events.is_empty()
    }

    /// Updates tree state from the new Rollup operations blocks, saves it in storage
    ///
    /// # Arguments
    ///
    /// * `new_ops_blocks` - the new Rollup operations blocks
    ///
    fn update_tree_state(&mut self, new_ops_blocks: Vec<RollupOpsBlock>) {
        let mut blocks = vec![];
        let mut updates = vec![];
        let mut count = 0;
        for op_block in new_ops_blocks {
            let (block, acc_updates) = self
                .tree_state
                .update_tree_states_from_ops_block(&op_block)
                .expect("Updating tree state: cant update tree from operations");
            blocks.push(block);
            updates.push(acc_updates);
            count += 1;
        }
        for i in 0..count {
            storage_interactor::update_tree_state(
                &self.connection_pool,
                blocks[i].clone(),
                updates[i].clone(),
            );
        }

        debug!("Updated state");
    }

    /// Gets new operations blocks from events, updates rollup operations stored state.
    /// Returns new rollup operations blocks
    fn update_operations_state(&mut self) -> Vec<RollupOpsBlock> {
        let new_blocks = self.get_new_operation_blocks_from_events();

        storage_interactor::save_rollup_ops(&self.connection_pool, &new_blocks);

        debug!("Updated operations storage");

        new_blocks
    }

    /// Returns verified comitted operations blocks from verified op blocks events
    pub fn get_new_operation_blocks_from_events(&mut self) -> Vec<RollupOpsBlock> {
        self.events_state
            .get_only_verified_committed_events()
            .iter()
            .map(|event| {
                RollupOpsBlock::get_rollup_ops_block(&self.web3, &event)
                    .expect("Cant get new operation blocks from events")
            })
            .collect()
    }
}
