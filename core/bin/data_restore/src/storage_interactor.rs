use std::{collections::HashMap, convert::TryFrom};

use web3::types::H256;

use zksync_storage::data_restore::records::{
    NewBlockEvent, StoredBlockEvent, StoredRollupOpsBlock,
};
use zksync_types::withdrawals::{WithdrawalEvent, WithdrawalPendingEvent};
use zksync_types::{
    block::Block, AccountId, AccountMap, AccountUpdate, AccountUpdates, BlockNumber, NewTokenEvent,
    PriorityOp, SerialId, Token, TokenId, TokenInfo, NFT,
};

use crate::{
    contract::ZkSyncContractVersion,
    data_restore_driver::StorageUpdateState,
    database_storage_interactor::DatabaseStorageInteractor,
    events::{BlockEvent, EventType},
    events_state::EventsState,
    inmemory_storage_interactor::InMemoryStorageInteractor,
    rollup_ops::RollupOpsBlock,
};

pub struct StoredTreeState {
    pub last_block_number: BlockNumber,
    pub account_map: AccountMap,
    pub unprocessed_prior_ops: u64,
    pub fee_acc_id: AccountId,
}

pub struct CachedTreeState {
    pub tree_cache: serde_json::Value,
    pub account_map: AccountMap,
    pub current_block: Block,
    pub nfts: HashMap<TokenId, NFT>,
}

#[allow(clippy::large_enum_variant)]
pub enum StorageInteractor<'a> {
    Database(DatabaseStorageInteractor<'a>),
    InMemory(InMemoryStorageInteractor),
}

macro_rules! storage_interact {
    ($obj:ident.$method:ident($($args:ident),*)) => {
        match $obj {
            StorageInteractor::Database(db) => db.$method($($args),*).await,
            StorageInteractor::InMemory(db) => db.$method($($args),*).await,
        }
    }
}

impl StorageInteractor<'_> {
    pub async fn start_transaction<'c: 'b, 'b>(&'c mut self) -> StorageInteractor<'b> {
        match self {
            StorageInteractor::Database(db) => {
                let transaction = db.start_transaction().await;
                StorageInteractor::Database(transaction)
            }
            StorageInteractor::InMemory(db) => {
                let transaction = db.start_transaction().await;
                StorageInteractor::InMemory(transaction)
            }
        }
    }

    pub async fn commit(self) {
        storage_interact!(self.commit())
    }

    /// Saves Rollup operations blocks in storage
    ///
    /// # Arguments
    ///
    /// * `blocks` - Rollup operations blocks
    ///
    pub async fn save_rollup_ops(&mut self, blocks: &[RollupOpsBlock]) {
        storage_interact!(self.save_rollup_ops(blocks))
    }

    /// Updates stored tree state: saves block transactions in storage, stores blocks and account updates
    ///
    /// # Arguments
    ///
    /// * `block` - Rollup block
    /// * `accounts_updated` - accounts updates
    ///
    pub async fn update_tree_state(&mut self, block: Block, accounts_updated: AccountUpdates) {
        storage_interact!(self.update_tree_state(block, accounts_updated))
    }

    /// Saves the priority operations metadata in storage.
    ///
    /// # Arguments
    ///
    /// * `priority_op_data` - Priority operations.
    ///
    /// # Returns
    ///
    /// Ids of operations with no corresponding block in storage yet.
    /// These should not be removed from the events state until the next
    /// Ethereum block range.
    ///
    pub async fn apply_priority_op_data(
        &mut self,
        priority_op_data: impl Iterator<Item = &PriorityOp>,
    ) -> Vec<SerialId> {
        storage_interact!(self.apply_priority_op_data(priority_op_data))
    }

    /// Store token to the storage  
    /// # Arguments
    ///
    /// * `token` - Token that added when deploying contract
    /// * `token_id` - Id for token in our system
    ///
    pub async fn store_token(&mut self, token: TokenInfo, token_id: TokenId) {
        storage_interact!(self.store_token(token, token_id))
    }

    /// Saves Rollup contract events in storage (includes block events, new tokens and last watched eth block number)
    ///
    /// # Arguments
    ///
    /// * `block_events` - Rollup contract block events descriptions
    /// * `tokens` - Tokens that had been added to system
    /// * `last_watched_eth_block_number` - Last watched ethereum block
    ///
    pub async fn save_events_state(
        &mut self,
        block_events: &[BlockEvent],
        tokens: &[NewTokenEvent],
        priority_op_data: &[PriorityOp],
        last_watched_eth_block_number: u64,
    ) {
        storage_interact!(self.save_events_state(
            block_events,
            tokens,
            priority_op_data,
            last_watched_eth_block_number
        ))
    }

    pub async fn save_withdrawals(
        &mut self,
        withdrawals: &[WithdrawalEvent],
        pending_withdrawals: &[WithdrawalPendingEvent],
    ) {
        storage_interact!(self.save_withdrawals(withdrawals, pending_withdrawals))
    }

    /// Saves genesis accounts state in storage
    ///
    /// # Arguments
    ///
    /// * `genesis_updates` - Genesis account updates
    ///
    pub async fn save_genesis_tree_state(
        &mut self,
        genesis_updates: &[(AccountId, AccountUpdate)],
    ) {
        storage_interact!(self.save_genesis_tree_state(genesis_updates))
    }

    /// Saves special NFT token in storage
    ///
    /// # Arguments
    ///
    /// * `token` - Special token to be stored
    ///
    pub async fn save_special_token(&mut self, token: Token) {
        storage_interact!(self.save_special_token(token))
    }

    /// Returns Rollup contract events state from storage
    pub async fn get_block_events_state_from_storage(&mut self) -> EventsState {
        storage_interact!(self.get_block_events_state_from_storage())
    }

    /// Returns the current Rollup block, tree accounts map, unprocessed priority ops and the last fee acc from storage
    pub async fn get_tree_state(&mut self) -> StoredTreeState {
        storage_interact!(self.get_tree_state())
    }

    /// Returns Rollup operations blocks from storage
    pub async fn get_ops_blocks_from_storage(&mut self) -> Vec<RollupOpsBlock> {
        storage_interact!(self.get_ops_blocks_from_storage())
    }

    /// Updates the `eth_stats` table with the currently last available committed/verified blocks
    /// data for `eth_sender` module to operate correctly.
    pub async fn update_eth_state(&mut self) {
        storage_interact!(self.update_eth_state())
    }

    /// Returns last recovery state update step from storage
    pub async fn get_storage_state(&mut self) -> StorageUpdateState {
        storage_interact!(self.get_storage_state())
    }

    /// Returns cached tree state from storage. It's expected to be valid
    /// after completing `finite` restore mode and may be used to speed up the
    /// `continue` mode.
    pub async fn get_cached_tree_state(&mut self) -> Option<CachedTreeState> {
        storage_interact!(self.get_cached_tree_state())
    }

    /// Deletes the latest tree cache in the database and saves the new one.
    ///
    /// # Arguments
    ///
    /// * `block_number` - The corresponding block number
    /// * `tree_cache` - Merkle tree cache
    ///
    pub async fn update_tree_cache(&mut self, block_number: BlockNumber, tree_cache: String) {
        storage_interact!(self.update_tree_cache(block_number, tree_cache))
    }

    /// Retrieves the maximum serial id of a priority requests
    pub async fn get_max_priority_op_serial_id(&mut self) -> SerialId {
        storage_interact!(self.get_max_priority_op_serial_id())
    }
}

/// Returns Rollup contract event from its stored representation
///
/// # Arguments
///
/// * `block` - Stored representation of ZkSync Contract event
///
pub fn stored_block_event_into_block_event(block: StoredBlockEvent) -> BlockEvent {
    BlockEvent {
        block_num: BlockNumber(
            u32::try_from(block.block_num).expect("Wrong block number - cant convert into u32"),
        ),
        transaction_hash: H256::from_slice(block.transaction_hash.as_slice()),
        block_type: match &block.block_type {
            c if c == "Committed" => EventType::Committed,
            v if v == "Verified" => EventType::Verified,
            _ => panic!("Wrong block type"),
        },
        contract_version: ZkSyncContractVersion::try_from(block.contract_version as u32)
            .unwrap_or(ZkSyncContractVersion::V0),
    }
}

/// Get new stored representation of the Rollup contract event from itself
///
/// # Arguments
///
/// * `event` - Rollup contract event description
///
pub fn block_event_into_stored_block_event(event: &BlockEvent) -> NewBlockEvent {
    NewBlockEvent {
        block_type: match event.block_type {
            EventType::Committed => "Committed".to_string(),
            EventType::Verified => "Verified".to_string(),
        },
        transaction_hash: event.transaction_hash.as_bytes().to_vec(),
        block_num: i64::from(*event.block_num),
        contract_version: event.contract_version.into(),
    }
}

/// Returns Rollup operations block from its stored representation
///
/// # Arguments
///
/// * `op_block` - Stored ZkSync operations block description
///
pub fn stored_ops_block_into_ops_block(op_block: StoredRollupOpsBlock) -> RollupOpsBlock {
    RollupOpsBlock {
        block_num: BlockNumber::from(op_block.block_num as u32),
        ops: op_block
            .ops
            .unwrap_or_default()
            .into_iter()
            .map(|op| {
                serde_json::from_value(op)
                    .expect("couldn't deserialize `ZkSyncOp` from the database")
            })
            .collect(),
        fee_account: AccountId::from(op_block.fee_account as u32),
        timestamp: op_block.timestamp.map(|t| t as u64),
        previous_block_root_hash: op_block
            .previous_block_root_hash
            .map(|h| H256::from_slice(&h))
            .unwrap_or_default(),
        contract_version: Some(
            ZkSyncContractVersion::try_from(op_block.contract_version as u32)
                .expect("invalid contract version in the database"),
        ),
    }
}
