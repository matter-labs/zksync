// Built-in deps
use std::convert::TryFrom;
use std::str::FromStr;
// External deps
use failure::format_err;
use web3::types::H256;
// Workspace deps
use crate::data_restore_driver::StorageUpdateState;
use crate::events::{BlockEvent, EventType};
use crate::events_state::EventsState;
use crate::rollup_ops::RollupOpsBlock;
use models::node::block::{Block, ExecutedOperations};
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
) -> Result<(), failure::Error> {
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore save tree state: {}",
            e.to_string()
        )
    })?;
    let (_last_committed, mut _accounts) = storage.load_committed_state(None).expect("db failed");
    assert!(
        _last_committed == 0 && _accounts.is_empty(),
        "db should be empty"
    );
    storage
        .commit_state_update(0, &[(0, genesis_acc_update)])
        .map_err(|e| format_err!("Cant commit state update: {}", e.to_string()))?;
    storage
        .apply_state_update(0)
        .map_err(|e| format_err!("Cant apply state update: {}", e.to_string()))?;
    Ok(())
}

/// Saves tokens that had been added to system in storage
///
/// # Arguments
///
/// * `connection_pool` - Database connection pool
/// * `tokens` - Tokens that had been added to system
///
pub fn save_tokens(
    connection_pool: &ConnectionPool,
    tokens: Vec<TokenAddedEvent>,
) -> Result<(), failure::Error> {
    let storage = connection_pool
        .access_storage()
        .map_err(|e| format_err!("Db connection failed for saving token: {}", e.to_string()))?;
    for token in tokens {
        storage
            .store_token(token.id as u16, &format!("0x{:x}", token.address), None)
            .map_err(|e| format_err!("Cant store token: {}", e.to_string()))?;
    }
    Ok(())
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
) -> Result<(), failure::Error> {
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore save tree state: {}",
            e.to_string()
        )
    })?;

    for transaction in block.block_transactions.clone() {
        if let ExecutedOperations::Tx(tx) = transaction {
            storage
                .mempool_add_tx_unsafe(&tx.tx)
                .map_err(|e| format_err!("Cant save txs in mempool: {}", e.to_string()))?;
        }
    }

    if accounts_updated.is_empty() && block.number_of_processed_prior_ops() == 0 {
        storage
            .save_block_transactions(&block)
            .map_err(|e| format_err!("Cant save block transactions: {}", e.to_string()))?;
        return Ok(());
    }

    let commit_op = Operation {
        action: Action::Commit,
        block: block.clone(),
        accounts_updated: accounts_updated.clone(),
        id: None,
    };
    storage
        .execute_operation_data_restore(&commit_op)
        .map_err(|e| format_err!("Cant execute commit operation: {}", e.to_string()))?;

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
        .map_err(|e| format_err!("Cant execute verify operation: {}", e.to_string()))?;

    Ok(())
}

/// Saves Rollup contract events in storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
/// * `events` - Rollup contract events descriptions
///
pub fn save_block_events_state(
    connection_pool: &ConnectionPool,
    events: &[BlockEvent],
) -> Result<(), failure::Error> {
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore save events: {}",
            e.to_string()
        )
    })?;
    let mut new_events: Vec<NewBlockEvent> = vec![];
    for event in events {
        new_events.push(block_event_into_stored_block_event(event));
    }
    storage
        .delete_events_state()
        .map_err(|e| format_err!("No events state to delete: {}", e.to_string()))?;
    storage
        .save_events_state(new_events.as_slice())
        .map_err(|e| format_err!("Cant save events state: {}", e.to_string()))?;

    Ok(())
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
) -> Result<(), failure::Error> {
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore save events: {}",
            e.to_string()
        )
    })?;

    let block_number = NewLastWatchedEthBlockNumber {
        block_number: last_watched_eth_block_number.to_string(),
    };
    storage
        .delete_last_watched_block_number()
        .map_err(|e| format_err!("No last watched block number to delete: {}", e.to_string()))?;
    storage
        .save_last_watched_block_number(&block_number)
        .map_err(|e| format_err!("Cant save last watched block number: {}", e.to_string()))?;

    Ok(())
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
pub fn save_storage_state(
    connection_pool: &ConnectionPool,
    state: StorageUpdateState,
) -> Result<(), failure::Error> {
    let string = match state {
        StorageUpdateState::None => "None".to_string(),
        StorageUpdateState::Events => "Events".to_string(),
        StorageUpdateState::Operations => "Operations".to_string(),
    };
    let storage_state = NewStorageState {
        storage_state: string,
    };
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore save storage state: {}",
            e.to_string()
        )
    })?;
    storage
        .delete_data_restore_storage_state_status()
        .map_err(|e| format_err!("No storage state status to delete: {}", e.to_string()))?;
    storage
        .save_storage_state(&storage_state)
        .map_err(|e| format_err!("Cant save storage state: {}", e.to_string()))?;
    Ok(())
}

/// Saves Rollup operations blocks in storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
/// * `blocks` - Rollup operations blocks
///
pub fn save_rollup_ops(
    connection_pool: &ConnectionPool,
    blocks: &[RollupOpsBlock],
) -> Result<(), failure::Error> {
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore save ops blocks: {}",
            e.to_string()
        )
    })?;
    storage
        .delete_rollup_ops()
        .map_err(|e| format_err!("No franklin ops to delete: {}", e.to_string()))?;
    for block in blocks {
        storage
            .save_rollup_ops(block.ops.as_slice(), block.block_num, block.fee_account)
            .map_err(|e| format_err!("Cant save franklin transaction: {}", e.to_string()))?;
    }
    Ok(())
}

/// Returns Rollup operations blocks from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_ops_blocks_from_storage(
    connection_pool: &ConnectionPool,
) -> Result<Vec<RollupOpsBlock>, failure::Error> {
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore get ops blocks: {}",
            e.to_string()
        )
    })?;
    let committed_blocks = storage
        .load_rollup_ops_blocks()
        .map_err(|e| format_err!("No ops blocks to delete: {}", e.to_string()))?;
    let mut blocks: Vec<RollupOpsBlock> = vec![];
    for block in committed_blocks {
        blocks.push(stored_ops_block_into_ops_block(&block));
    }
    Ok(blocks)
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
pub fn get_storage_state(
    connection_pool: &ConnectionPool,
) -> Result<StorageUpdateState, failure::Error> {
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore get storage state: {}",
            e.to_string()
        )
    })?;

    let storage_state_string = storage
        .load_storage_state()
        .map_err(|e| format_err!("Load_storage_state: db must work: {}", e.to_string()))?
        .storage_state;

    let state = match storage_state_string.as_ref() {
        "Events" => StorageUpdateState::Events,
        "Operations" => StorageUpdateState::Operations,
        "None" => StorageUpdateState::None,
        _ => return Err(format_err!("Unknown storage state for data restores")),
    };

    Ok(state)
}

/// Returns last watched ethereum block number from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_last_watched_block_number_from_storage(
    connection_pool: &ConnectionPool,
) -> Result<u64, failure::Error> {
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for get last block number: {}",
            e.to_string()
        )
    })?;

    let last_watched_block_number_string = storage
        .load_last_watched_block_number()
        .map_err(|e| format_err!("Load_blocks_events: db must work: {}", e.to_string()))?
        .block_number;

    Ok(
        u64::from_str(last_watched_block_number_string.as_str()).map_err(|e| {
            format_err!(
                "Сant make u256 block_number in get_last_watched_block_number_from_storage: {}",
                e.to_string()
            )
        })?,
    )
}

/// Returns Rollup contract events state from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_block_events_state_from_storage(
    connection_pool: &ConnectionPool,
) -> Result<EventsState, failure::Error> {
    let last_watched_eth_block_number =
        get_last_watched_block_number_from_storage(&connection_pool)?;

    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for get past events: {}",
            e.to_string()
        )
    })?;

    let committed = storage
        .load_committed_events_state()
        .map_err(|e| format_err!("Load committed state failed: {}", e.to_string()))?;

    let mut committed_events: Vec<BlockEvent> = vec![];
    for event in committed {
        let block_event = stored_block_event_into_block_event(event.clone())?;
        committed_events.push(block_event);
    }

    let verified = storage
        .load_verified_events_state()
        .map_err(|e| format_err!("Db connection failed for past events: {}", e.to_string()))?;
    let mut verified_events: Vec<BlockEvent> = vec![];
    for event in verified {
        let block_event = stored_block_event_into_block_event(event.clone())?;
        verified_events.push(block_event);
    }

    Ok(EventsState {
        committed_events,
        verified_events,
        last_watched_eth_block_number,
    })
}

/// Returns Rollup contract event from its stored representation
///
/// # Arguments
///
/// * `block` - Stored representation of Franklin Contract event
///
pub fn stored_block_event_into_block_event(
    block: StoredBlockEvent,
) -> Result<BlockEvent, failure::Error> {
    Ok(BlockEvent {
        block_num: u32::try_from(block.block_num)?,
        transaction_hash: H256::from_slice(block.transaction_hash.as_slice()),
        block_type: match &block.block_type {
            c if c == "Committed" => EventType::Committed,
            v if v == "Verified" => EventType::Verified,
            _ => return Err(format_err!("Wrong block type: {}", &block.block_type)),
        },
    })
}

/// Returns the current Rollup block, tree accounts map, unprocessed priority ops and the last fee acc from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
/// connection_pool: &ConnectionPool,
pub fn get_tree_state(
    connection_pool: &ConnectionPool,
) -> Result<(u32, AccountMap, u64, u32), failure::Error> {
    let storage = connection_pool
        .access_storage()
        .map_err(|e| format_err!("Db connection failed for tree state: {}", e.to_string()))?;

    let verified_state_result = storage.load_verified_state();
    let (last_block, account_map) = match verified_state_result {
        Err(err) => {
            warn!("There are no last verified state in storage - will be updated from empty state. Reason: {:?}", err.to_string());
            (0, AccountMap::default())
        }
        Ok(res) => (res.0, res.1),
    };

    let block_result = storage.get_block(last_block);
    let block_opt = match block_result {
        Err(err) => {
            warn!(
                "Cant get last block from storage. Reason: {:?}",
                err.to_string()
            );
            None
        }
        Ok(res) => res,
    };
    let (unprocessed_prior_ops, fee_acc_id) = match block_opt {
        None => {
            warn!("There are no last block in storage");
            (0, 0)
        }
        Some(block) => (block.processed_priority_ops.1, block.fee_account),
    };

    Ok((last_block, account_map, unprocessed_prior_ops, fee_acc_id))
}
