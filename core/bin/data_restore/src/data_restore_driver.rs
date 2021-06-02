// External deps
use web3::{
    contract::Contract,
    types::{H160, H256},
    Transport, Web3,
};
// Workspace deps
use zksync_contracts::governance_contract;
use zksync_crypto::{
    params::{MIN_NFT_TOKEN_ID, NFT_STORAGE_ACCOUNT_ADDRESS, NFT_STORAGE_ACCOUNT_ID, NFT_TOKEN_ID},
    Fr,
};
use zksync_types::{Account, AccountId, AccountMap, AccountUpdate, BlockNumber, Token};

// Local deps
use crate::{
    contract::{get_genesis_account, ZkSyncDeployedContract},
    eth_tx_helpers::get_ethereum_transaction,
    events_state::EventsState,
    rollup_ops::RollupOpsBlock,
    storage_interactor::StorageInteractor,
    tree_state::TreeState,
};

use std::marker::PhantomData;

/// Storage state update:
/// - None - The state is updated completely last time - start from fetching the new events
/// - Events - The events fetched and saved successfully - now get operations from them and update tree
/// - Operations - There are operations that are not presented in the tree state - update tree state
#[derive(Debug, Copy, Clone)]
pub enum StorageUpdateState {
    None,
    Events,
    Operations,
}

/// Data restore driver is a high level interface for all restoring components.
/// It is actually a finite state machine, that has following states:
/// - Empty - The state is new
/// - None - The state is completely updated last time, driver will load state from storage and fetch new events
/// - Events - The events has been fetched and saved successfully and firstly driver will load state from storage
///   and get new operation for last saved events
/// - Operations - The operations and events has been fetched and saved successfully and firstly driver will load
///   state from storage and update merkle tree by last saved operations
///
/// Driver can interact with other restoring components for their updating:
/// - Events
/// - Operations
/// - Tree
/// - Storage
pub struct DataRestoreDriver<T: Transport, I> {
    /// Web3 provider endpoint
    pub web3: Web3<T>,
    /// Provides Ethereum Governance contract interface
    pub governance_contract: (ethabi::Contract, Contract<T>),
    /// Ethereum blocks that include correct UpgradeComplete events.
    /// Should be provided via config.
    pub contract_upgrade_eth_blocks: Vec<u64>,
    /// The initial version of the deployed zkSync contract.
    pub init_contract_version: u32,
    /// Provides Ethereum Rollup contract interface
    pub zksync_contract: ZkSyncDeployedContract<T>,
    /// Rollup contract events state
    pub events_state: EventsState,
    /// Rollup accounts state
    pub tree_state: TreeState,
    /// The step distance of viewing events in the ethereum blocks
    pub eth_blocks_step: u64,
    /// The distance to the last ethereum block
    pub end_eth_blocks_offset: u64,
    /// Finite mode flag. In finite mode, driver will only work until
    /// amount of restored blocks will become equal to amount of known
    /// verified blocks. After that, it will stop.
    pub finite_mode: bool,
    /// Expected root hash to be observed after restoring process. Only
    /// available in finite mode, and intended for tests.
    pub final_hash: Option<Fr>,
    phantom_data: PhantomData<I>,
}

impl<T, I> DataRestoreDriver<T, I>
where
    T: Transport,
    I: StorageInteractor,
{
    /// Returns new data restore driver with empty events and tree states.
    ///
    /// # Arguments
    ///
    /// * `web3_transport` - Web3 provider transport
    /// * `governance_contract_eth_addr` - Governance contract address
    /// * `upgrade_eth_blocks` - Ethereum blocks that include correct UpgradeComplete events
    /// * `init_contract_version` - The initial version of the deployed zkSync contract
    /// * `eth_blocks_step` - The step distance of viewing events in the ethereum blocks
    /// * `end_eth_blocks_offset` - The distance to the last ethereum block
    /// * `finite_mode` - Finite mode flag.
    /// * `final_hash` - Hash of the last block which we want to restore
    /// * `zksync_contract` - Current deployed zksync contract
    ///
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        web3: Web3<T>,
        governance_contract_eth_addr: H160,
        contract_upgrade_eth_blocks: Vec<u64>,
        init_contract_version: u32,
        eth_blocks_step: u64,
        end_eth_blocks_offset: u64,
        finite_mode: bool,
        final_hash: Option<Fr>,
        zksync_contract: ZkSyncDeployedContract<T>,
    ) -> Self {
        let governance_contract = {
            let abi = governance_contract();
            (
                abi.clone(),
                Contract::new(web3.eth(), governance_contract_eth_addr, abi),
            )
        };

        let events_state = EventsState::default();

        let tree_state = TreeState::new();
        Self {
            web3,
            governance_contract,
            contract_upgrade_eth_blocks,
            init_contract_version,
            zksync_contract,
            events_state,
            tree_state,
            eth_blocks_step,
            end_eth_blocks_offset,
            finite_mode,
            final_hash,
            phantom_data: Default::default(),
        }
    }

    /// Sets the 'genesis' state.
    /// Tree with inserted genesis account will be created.
    /// Used when restore driver is restarted.
    ///
    /// # Arguments
    ///
    /// * `governance_contract_genesis_tx_hash` - Governance contract creation tx hash
    ///
    pub async fn set_genesis_state(&mut self, interactor: &mut I, genesis_tx_hash: H256) {
        let genesis_transaction = get_ethereum_transaction(&self.web3, &genesis_tx_hash)
            .await
            .expect("Cant get zkSync genesis transaction");

        // Setting genesis block number for events state
        let genesis_eth_block_number = self
            .events_state
            .set_genesis_block_number(&genesis_transaction)
            .expect("Cant set genesis block number for events state");
        vlog::info!("genesis_eth_block_number: {:?}", &genesis_eth_block_number);

        interactor
            .save_events_state(&[], &[], genesis_eth_block_number)
            .await;

        let genesis_fee_account =
            get_genesis_account(&genesis_transaction).expect("Cant get genesis account address");

        vlog::info!(
            "genesis fee account address: 0x{}",
            hex::encode(genesis_fee_account.address.as_ref())
        );

        interactor
            .save_special_token(Token {
                id: NFT_TOKEN_ID,
                symbol: "SPECIAL".to_string(),
                address: *NFT_STORAGE_ACCOUNT_ADDRESS,
                decimals: 18,
                is_nft: true,
            })
            .await;
        vlog::info!("Special token added");

        let mut account_updates = Vec::with_capacity(3);
        let mut account_map = AccountMap::default();

        account_updates.push((
            AccountId(0),
            AccountUpdate::Create {
                address: genesis_fee_account.address,
                nonce: genesis_fee_account.nonce,
            },
        ));
        account_map.insert(AccountId(0), genesis_fee_account);

        let (mut special_account, special_account_create) =
            Account::create_account(NFT_STORAGE_ACCOUNT_ID, *NFT_STORAGE_ACCOUNT_ADDRESS);
        special_account.set_balance(NFT_TOKEN_ID, num::BigUint::from(MIN_NFT_TOKEN_ID));

        account_updates.push(special_account_create[0].clone());
        account_updates.push((
            NFT_STORAGE_ACCOUNT_ID,
            AccountUpdate::UpdateBalance {
                old_nonce: special_account.nonce,
                new_nonce: special_account.nonce,
                balance_update: (
                    NFT_TOKEN_ID,
                    num::BigUint::from(0u64),
                    num::BigUint::from(MIN_NFT_TOKEN_ID),
                ),
            },
        ));
        account_map.insert(NFT_STORAGE_ACCOUNT_ID, special_account);

        let current_block = BlockNumber(0);
        let current_unprocessed_priority_op = 0;
        let fee_acc_num = 0;

        let tree_state = TreeState::load(
            current_block,
            account_map,
            current_unprocessed_priority_op,
            AccountId(fee_acc_num),
        );

        vlog::info!("Genesis tree root hash: {:?}", tree_state.root_hash());
        vlog::debug!("Genesis accounts: {:?}", tree_state.get_accounts());

        interactor.save_genesis_tree_state(&account_updates).await;

        vlog::info!("Saved genesis tree state\n");

        self.tree_state = tree_state;
    }

    /// Stops states from storage
    pub async fn load_state_from_storage(&mut self, interactor: &mut I) -> bool {
        vlog::info!("Loading state from storage");
        let state = interactor.get_storage_state().await;
        self.events_state = interactor.get_block_events_state_from_storage().await;
        let tree_state = interactor.get_tree_state().await;
        self.tree_state = TreeState::load(
            tree_state.last_block_number,     // current block
            tree_state.account_map,           // account map
            tree_state.unprocessed_prior_ops, // unprocessed priority op
            tree_state.fee_acc_id,            // fee account
        );
        match state {
            StorageUpdateState::Events => {
                // Update operations
                let new_ops_blocks = self.update_operations_state(interactor).await;
                // Update tree
                self.update_tree_state(interactor, new_ops_blocks).await;
            }
            StorageUpdateState::Operations => {
                // Update operations
                let new_ops_blocks = interactor.get_ops_blocks_from_storage().await;
                // Update tree
                self.update_tree_state(interactor, new_ops_blocks).await;
            }
            StorageUpdateState::None => {}
        }
        let total_verified_blocks = self.zksync_contract.get_total_verified_blocks().await;

        let last_verified_block = self.tree_state.state.block_number;
        vlog::info!(
            "State has been loaded\nProcessed {:?} blocks on contract\nRoot hash: {:?}\n",
            last_verified_block,
            self.tree_state.root_hash()
        );

        self.finite_mode && (total_verified_blocks == *last_verified_block)
    }

    /// Activates states updates
    pub async fn run_state_update(&mut self, interactor: &mut I) {
        let mut last_watched_block: u64 = self.events_state.last_watched_eth_block_number;
        let mut final_hash_was_found = false;
        loop {
            vlog::info!("Last watched ethereum block: {:?}", last_watched_block);

            // Update events
            if self.update_events_state(interactor).await {
                // Update operations
                let new_ops_blocks = self.update_operations_state(interactor).await;

                if !new_ops_blocks.is_empty() {
                    // Update tree
                    self.update_tree_state(interactor, new_ops_blocks).await;

                    let total_verified_blocks =
                        self.zksync_contract.get_total_verified_blocks().await;

                    let last_verified_block = self.tree_state.state.block_number;

                    // We must update the Ethereum stats table to match the actual stored state
                    // to keep the `state_keeper` consistent with the `eth_sender`.
                    interactor.update_eth_state().await;

                    vlog::info!(
                        "State updated\nProcessed {:?} blocks of total {:?} verified on contract\nRoot hash: {:?}\n",
                        last_verified_block,
                        total_verified_blocks,
                        self.tree_state.root_hash()
                    );

                    // If there is an expected root hash, check if current root hash matches the observed
                    // one.
                    // We check it after every block, since provided final hash may be not the latest hash
                    // by the time when it was processed.
                    if let Some(root_hash) = self.final_hash {
                        if root_hash == self.tree_state.root_hash() {
                            final_hash_was_found = true;
                            vlog::info!(
                                "Correct expected root hash was met on the block {} out of {}",
                                *last_verified_block,
                                total_verified_blocks
                            );
                        }
                    }

                    if self.finite_mode && *last_verified_block == total_verified_blocks {
                        // Check if the final hash was found and panic otherwise.
                        if self.final_hash.is_some() && !final_hash_was_found {
                            panic!("Final hash was not met during the state restoring process");
                        }

                        // We've restored all the blocks, our job is done.
                        break;
                    }
                }
            }

            if last_watched_block == self.events_state.last_watched_eth_block_number {
                vlog::info!("sleep block");
                std::thread::sleep(std::time::Duration::from_secs(5));
            } else {
                last_watched_block = self.events_state.last_watched_eth_block_number;
            }
        }
    }

    /// Updates events state, saves new blocks, tokens events and the last watched eth block number in storage
    /// Returns bool flag, true if there are new block events
    async fn update_events_state(&mut self, interactor: &mut I) -> bool {
        let (block_events, token_events, last_watched_eth_block_number) = self
            .events_state
            .update_events_state(
                &self.web3,
                &self.zksync_contract,
                &self.governance_contract,
                &self.contract_upgrade_eth_blocks,
                self.eth_blocks_step,
                self.end_eth_blocks_offset,
                self.init_contract_version,
            )
            .await
            .expect("Updating events state: cant update events state");
        interactor
            .save_events_state(
                &block_events,
                token_events.as_slice(),
                last_watched_eth_block_number,
            )
            .await;

        !block_events.is_empty()
    }

    /// Updates tree state from the new Rollup operations blocks, saves it in storage
    ///
    /// # Arguments
    ///
    /// * `new_ops_blocks` - the new Rollup operations blocks
    ///
    async fn update_tree_state(&mut self, interactor: &mut I, new_ops_blocks: Vec<RollupOpsBlock>) {
        let mut blocks = vec![];
        let mut updates = vec![];
        let mut count = 0;
        for op_block in new_ops_blocks {
            // Take the contract version into account when choosing block chunk sizes.
            let available_block_chunk_sizes = op_block
                .contract_version
                .expect("contract version must be set")
                .available_block_chunk_sizes();
            let (block, acc_updates) = self
                .tree_state
                .update_tree_states_from_ops_block(&op_block, available_block_chunk_sizes)
                .expect("Updating tree state: cant update tree from operations");
            blocks.push(block);
            updates.push(acc_updates);
            count += 1;
        }
        for i in 0..count {
            interactor
                .update_tree_state(blocks[i].clone(), updates[i].clone())
                .await;
        }

        vlog::debug!("Updated state");
    }

    /// Gets new operations blocks from events, updates rollup operations stored state.
    /// Returns new rollup operations blocks
    async fn update_operations_state(&mut self, interactor: &mut I) -> Vec<RollupOpsBlock> {
        let new_blocks = self.get_new_operation_blocks_from_events().await;

        interactor.save_rollup_ops(&new_blocks).await;

        vlog::debug!("Updated operations storage");

        new_blocks
    }

    /// Returns verified comitted operations blocks from verified op blocks events
    pub async fn get_new_operation_blocks_from_events(&mut self) -> Vec<RollupOpsBlock> {
        let mut blocks = Vec::new();

        let mut last_event_tx_hash = None;
        for event in self
            .events_state
            .get_only_verified_committed_events()
            .iter()
        {
            // We use an aggregated block in contracts, which means that several BlockEvent can include the same tx_hash,
            // but for correct restore we need to generate RollupBlocks from this tx only once.
            // These blocks go one after the other, and checking only the last transaction hash is safe
            if let Some(tx) = last_event_tx_hash {
                if tx == event.transaction_hash {
                    continue;
                }
            }

            let block = RollupOpsBlock::get_rollup_ops_blocks(&self.web3, &event)
                .await
                .expect("Cant get new operation blocks from events");
            blocks.extend(block);
            last_event_tx_hash = Some(event.transaction_hash);
        }

        blocks
    }
}
