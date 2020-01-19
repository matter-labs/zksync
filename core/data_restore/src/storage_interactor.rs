// Built-in deps
use std::convert::TryFrom;
use std::str::FromStr;
// External deps
use web3::types::H256;
// Workspace deps
use crate::data_restore_driver::StorageUpdateState;
use crate::events::{BlockEvent, EventType};
use crate::events_state::EventsState;
use crate::rollup_ops::RollupOpsBlock;
use models::node::block::Block;
use models::node::{AccountMap, AccountUpdate, AccountUpdates};
use models::TokenAddedEvent;
use models::{Action, EncodedProof, Operation};
use storage::{
    ConnectionPool, NewBlockEvent, NewLastWatchedEthBlockNumber, NewStorageState, StoredBlockEvent,
    StoredRollupOpsBlock,
};

/// Saves genesis account state in storage
///
/// # Arguments
///
/// * `connection_pool` - Database connection pool
/// * `genesis_acc_update` - Genesis account update
///
pub fn save_genesis_tree_state(
    connection_pool: &ConnectionPool,
    genesis_acc_update: AccountUpdate,
) {
    let storage = connection_pool.access_storage().expect("db failed");
    let (_last_committed, mut _accounts) = storage
        .load_committed_state(None)
        .expect("Cant load comitted state");
    assert!(
        _last_committed == 0 && _accounts.is_empty(),
        "db should be empty"
    );
    storage
        .commit_state_update(0, &[(0, genesis_acc_update)])
        .expect("Cant commit tree state update");
    storage
        .apply_state_update(0)
        .expect("Cant apply tree state update");
}

/// Saves tokens that had been added to system in storage
///
/// # Arguments
///
/// * `connection_pool` - Database connection pool
/// * `tokens` - Tokens that had been added to system
///
pub fn save_tokens(connection_pool: &ConnectionPool, tokens: Vec<TokenAddedEvent>) {
    let storage = connection_pool.access_storage().expect("db failed");
    for token in tokens {
        storage
            .store_token(token.id as u16, &format!("0x{:x}", token.address), None)
            .expect("Cant store token");
    }
}

/// Updates stored tree state: saves block transactions in storage mempool, stores blocks and account updates
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
/// * `block` - Rollup block
/// * `accounts_updated` - accounts updates
///
pub fn update_tree_state(
    connection_pool: &ConnectionPool,
    block: Block,
    accounts_updated: AccountUpdates,
) {
    let storage = connection_pool.access_storage().expect("db failed");

    if accounts_updated.is_empty() && block.number_of_processed_prior_ops() == 0 {
        storage
            .save_block_transactions(&block)
            .expect("Cant save block transactions");
    } else {
        let commit_op = Operation {
            action: Action::Commit,
            block: block.clone(),
            accounts_updated: accounts_updated.clone(),
            id: None,
        };
        storage
            .execute_operation_data_restore(&commit_op)
            .expect("Cant execute commit operation");

        let verify_op = Operation {
            action: Action::Verify {
                proof: Box::new(EncodedProof::default()),
            },
            block,
            accounts_updated: Vec::new(),
            id: None,
        };
        storage
            .execute_operation_data_restore(&verify_op)
            .expect("Cant execute verify operation");
    }
}

/// Saves Rollup contract events in storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
/// * `events` - Rollup contract events descriptions
///
pub fn save_block_events_state(connection_pool: &ConnectionPool, events: &[BlockEvent]) {
    let storage = connection_pool.access_storage().expect("db failed");
    let mut new_events: Vec<NewBlockEvent> = vec![];
    for event in events {
        new_events.push(block_event_into_stored_block_event(event));
    }
    storage
        .update_events_state(new_events.as_slice())
        .expect("Cant update events state");
}

/// Saves last watched ethereum block number in storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
/// * `last_watched_eth_block_number` - Last watched ethereum block
///
pub fn save_last_watched_block_number(
    connection_pool: &ConnectionPool,
    last_watched_eth_block_number: u64,
) {
    let storage = connection_pool.access_storage().expect("db failed");

    let block_number = NewLastWatchedEthBlockNumber {
        block_number: last_watched_eth_block_number.to_string(),
    };
    storage
        .update_last_watched_block_number(&block_number)
        .expect("Cant update last watched block number");
}

/// Get new stored representation of the Rollup contract event from itself
///
/// # Arguments
///
/// * `evnet` - Rollup contract event description
///
pub fn block_event_into_stored_block_event(event: &BlockEvent) -> NewBlockEvent {
    NewBlockEvent {
        block_type: match event.block_type {
            EventType::Committed => "Committed".to_string(),
            EventType::Verified => "Verified".to_string(),
        },
        transaction_hash: event.transaction_hash.as_bytes().to_vec(),
        block_num: i64::from(event.block_num),
    }
}

/// Saves last recovery state update step
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
/// * `state` - last recovery state update step
///
pub fn save_storage_state(connection_pool: &ConnectionPool, state: StorageUpdateState) {
    let string = match state {
        StorageUpdateState::None => "None".to_string(),
        StorageUpdateState::Events => "Events".to_string(),
        StorageUpdateState::Operations => "Operations".to_string(),
    };
    let storage_state = NewStorageState {
        storage_state: string,
    };
    let storage = connection_pool.access_storage().expect("db failed");
    storage
        .update_storage_state(&storage_state)
        .expect("Cant update storage state status");
}

/// Saves Rollup operations blocks in storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
/// * `blocks` - Rollup operations blocks
///
pub fn save_rollup_ops(connection_pool: &ConnectionPool, blocks: &[RollupOpsBlock]) {
    let storage = connection_pool.access_storage().expect("db failed");
    storage.delete_rollup_ops().expect("Cant delete operations");
    for block in blocks {
        storage
            .save_rollup_ops(block.ops.as_slice(), block.block_num, block.fee_account)
            .expect("Cant save operations");
    }
}

/// Returns Rollup operations blocks from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_ops_blocks_from_storage(connection_pool: &ConnectionPool) -> Vec<RollupOpsBlock> {
    let storage = connection_pool.access_storage().expect("db failed");
    storage
        .load_rollup_ops_blocks()
        .expect("Cant load operation blocks")
        .iter()
        .map(|block| stored_ops_block_into_ops_block(&block))
        .collect()
}

/// Returns Rollup operations block from its stored representation
///
/// # Arguments
///
/// * `op_block` - Stored Franklin operations block description
///
pub fn stored_ops_block_into_ops_block(op_block: &StoredRollupOpsBlock) -> RollupOpsBlock {
    RollupOpsBlock {
        block_num: op_block.block_num,
        ops: op_block.ops.clone(),
        fee_account: op_block.fee_account,
    }
}

/// Returns last recovery state update step from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_storage_state(connection_pool: &ConnectionPool) -> StorageUpdateState {
    let storage = connection_pool.access_storage().expect("db failed");

    let storage_state_string = storage
        .load_storage_state()
        .expect("Cant load storage state")
        .storage_state;

    match storage_state_string.as_ref() {
        "Events" => StorageUpdateState::Events,
        "Operations" => StorageUpdateState::Operations,
        "None" => StorageUpdateState::None,
        _ => panic!("Unknown storage state"),
    }
}

/// Returns last watched ethereum block number from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_last_watched_block_number_from_storage(connection_pool: &ConnectionPool) -> u64 {
    let storage = connection_pool.access_storage().expect("db failed");

    let last_watched_block_number_string = storage
        .load_last_watched_block_number()
        .expect("Cant load last watched block number")
        .block_number;

    u64::from_str(last_watched_block_number_string.as_str())
        .expect("Сant make u256 block_number in get_last_watched_block_number_from_storage")
}

/// Returns Rollup contract events state from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_block_events_state_from_storage(connection_pool: &ConnectionPool) -> EventsState {
    let last_watched_eth_block_number =
        get_last_watched_block_number_from_storage(&connection_pool);

    let storage = connection_pool.access_storage().expect("db failed");

    let committed = storage
        .load_committed_events_state()
        .expect("Cant load committed state");

    let mut committed_events: Vec<BlockEvent> = vec![];
    for event in committed {
        let block_event = stored_block_event_into_block_event(event.clone());
        committed_events.push(block_event);
    }

    let verified = storage
        .load_verified_events_state()
        .expect("Cant load verified state");
    let mut verified_events: Vec<BlockEvent> = vec![];
    for event in verified {
        let block_event = stored_block_event_into_block_event(event.clone());
        verified_events.push(block_event);
    }

    EventsState {
        committed_events,
        verified_events,
        last_watched_eth_block_number,
    }
}

/// Returns Rollup contract event from its stored representation
///
/// # Arguments
///
/// * `block` - Stored representation of Franklin Contract event
///
pub fn stored_block_event_into_block_event(block: StoredBlockEvent) -> BlockEvent {
    BlockEvent {
        block_num: u32::try_from(block.block_num)
            .expect("Wrong block number - cant convert into u32"),
        transaction_hash: H256::from_slice(block.transaction_hash.as_slice()),
        block_type: match &block.block_type {
            c if c == "Committed" => EventType::Committed,
            v if v == "Verified" => EventType::Verified,
            _ => panic!("Wrong block type"),
        },
    }
}

/// Returns the current Rollup block, tree accounts map, unprocessed priority ops and the last fee acc from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
/// connection_pool: &ConnectionPool,
pub fn get_tree_state(connection_pool: &ConnectionPool) -> (u32, AccountMap, u64, u32) {
    let storage = connection_pool.access_storage().expect("db failed");

    let (last_block, account_map) = storage
        .load_verified_state()
        .expect("There are no last verified state in storage");

    let block = storage
        .get_block(last_block)
        .expect("Cant get the last block from storage")
        .expect("There are no last block in storage - restart driver");
    let (unprocessed_prior_ops, fee_acc_id) = (block.processed_priority_ops.1, block.fee_account);

    (last_block, account_map, unprocessed_prior_ops, fee_acc_id)
}
