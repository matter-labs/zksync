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
use models::node::{AccountMap, FranklinOp, AccountUpdate, AccountUpdates};
use models::TokenAddedEvent;
use models::{Action, EncodedProof, Operation};
use storage::{
    ConnectionPool, NewBlockEvent, NewLastWatchedEthBlockNumber, StoredBlockEvent,
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
        .save_genesis_state(genesis_acc_update)
        .expect("Cant update genesis state");
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
            .save_block_transactions_with_data_restore_state(&block)
            .expect("Cant save block transactions");
    } else {
        let commit_op = Operation {
            action: Action::Commit,
            block: block.clone(),
            accounts_updated: accounts_updated.clone(),
            id: None,
        };

        let verify_op = Operation {
            action: Action::Verify {
                proof: Box::new(EncodedProof::default()),
            },
            block,
            accounts_updated: Vec::new(),
            id: None,
        };

        storage
            .save_block_operations_with_data_restore_state(&commit_op, &verify_op)
            .expect("Cant execute verify operation");
    }
}

/// Saves Rollup contract events in storage (includes block events, new tokens and last watched eth block number)
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
/// * `eveblock_eventsnts` - Rollup contract block events descriptions
/// * `tokens` - Tokens that had been added to system
/// * `last_watched_eth_block_number` - Last watched ethereum block
///
pub fn save_events_state(
    connection_pool: &ConnectionPool,
    block_events: &[BlockEvent],
    tokens: &[TokenAddedEvent],
    last_watched_eth_block_number: u64,
) {
    let storage = connection_pool.access_storage().expect("db failed");

    let mut new_events: Vec<NewBlockEvent> = vec![];
    for event in block_events {
        new_events.push(block_event_into_stored_block_event(event));
    }

    let block_number = NewLastWatchedEthBlockNumber {
        block_number: last_watched_eth_block_number.to_string(),
    };

    storage
        .save_events_state_with_data_restore_state(
            new_events.as_slice(),
            tokens,
            &block_number,
        )
        .expect("Cant update events state");
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

/// Saves Rollup operations blocks in storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
/// * `blocks` - Rollup operations blocks
///
pub fn save_rollup_ops(connection_pool: &ConnectionPool, blocks: &[RollupOpsBlock]) {
    let storage = connection_pool.access_storage().expect("db failed");
    let mut ops: Vec<(u32, &FranklinOp, u32)> = vec![];

    for block in blocks {
        for op in &block.ops {
            ops.push((block.block_num, op, block.fee_account));
        }
    }

    storage
        .save_rollup_ops_with_data_restore_state(ops.as_slice())
        .expect("Cant update rollup operations");
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
