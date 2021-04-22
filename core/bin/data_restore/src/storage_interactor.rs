use std::convert::TryFrom;

use web3::types::H256;

use zksync_storage::data_restore::records::{
    NewBlockEvent, StoredBlockEvent, StoredRollupOpsBlock,
};
use zksync_types::{
    block::Block, AccountId, AccountMap, AccountUpdate, AccountUpdates, BlockNumber,
    TokenGenesisListItem, TokenId,
};

use crate::{
    contract::ZkSyncContractVersion,
    data_restore_driver::StorageUpdateState,
    events::{BlockEvent, EventType},
    events_state::{EventsState, NewTokenEvent},
    rollup_ops::RollupOpsBlock,
};

pub struct StoredTreeState {
    pub last_block_number: BlockNumber,
    pub account_map: AccountMap,
    pub unprocessed_prior_ops: u64,
    pub fee_acc_id: AccountId,
}

#[async_trait::async_trait]
pub trait StorageInteractor {
    /// Saves Rollup operations blocks in storage
    ///
    /// # Arguments
    ///
    /// * `blocks` - Rollup operations blocks
    ///
    async fn save_rollup_ops(&mut self, blocks: &[RollupOpsBlock]);

    /// Updates stored tree state: saves block transactions in storage, stores blocks and account updates
    ///
    /// # Arguments
    ///
    /// * `block` - Rollup block
    /// * `accounts_updated` - accounts updates
    ///
    async fn update_tree_state(&mut self, block: Block, accounts_updated: AccountUpdates);

    /// Store token to the storage  
    /// # Arguments
    ///
    /// * `token` - Token that added when deploying contract
    /// * `token_id` - Id for token in our system
    ///
    async fn store_token(&mut self, token: TokenGenesisListItem, token_id: TokenId);

    /// Saves Rollup contract events in storage (includes block events, new tokens and last watched eth block number)
    ///
    /// # Arguments
    ///
    /// * `eveblock_eventsnts` - Rollup contract block events descriptions
    /// * `tokens` - Tokens that had been added to system
    /// * `last_watched_eth_block_number` - Last watched ethereum block
    ///
    async fn save_events_state(
        &mut self,
        block_events: &[BlockEvent],
        tokens: &[NewTokenEvent],
        last_watched_eth_block_number: u64,
    );

    /// Saves genesis account state in storage
    ///
    /// # Arguments
    ///
    /// * `genesis_acc_update` - Genesis account update
    ///
    async fn save_genesis_tree_state(&mut self, genesis_acc_update: AccountUpdate);

    /// Returns Rollup contract events state from storage
    async fn get_block_events_state_from_storage(&mut self) -> EventsState;

    /// Returns the current Rollup block, tree accounts map, unprocessed priority ops and the last fee acc from storage
    async fn get_tree_state(&mut self) -> StoredTreeState;

    /// Returns Rollup operations blocks from storage
    async fn get_ops_blocks_from_storage(&mut self) -> Vec<RollupOpsBlock>;

    /// Updates the `eth_stats` table with the currently last available committed/verified blocks
    /// data for `eth_sender` module to operate correctly.
    async fn update_eth_state(&mut self);

    /// Returns last recovery state update step from storage
    async fn get_storage_state(&mut self) -> StorageUpdateState;
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
pub fn stored_ops_block_into_ops_block(op_block: &StoredRollupOpsBlock) -> RollupOpsBlock {
    RollupOpsBlock {
        block_num: op_block.block_num,
        ops: op_block.ops.clone(),
        fee_account: op_block.fee_account,
        timestamp: op_block.timestamp,
        previous_block_root_hash: op_block.previous_block_root_hash,
    }
}
