// Built-in uses
use std::convert::TryFrom;
use std::str::FromStr;
// External uses
use failure::format_err;
use web3::types::H256;
// Workspace uses
use crate::data_restore_driver::StorageUpdateState;
use crate::events::{EventData, EventType};
use crate::events_state::EventsState;
use crate::franklin_ops::FranklinOpsBlock;
use models::node::{AccountMap, AccountUpdate};
use storage::{
    ConnectionPool, NewBlockEvent, NewLastWatchedEthBlockNumber, NewStorageState, StoredBlockEvent,
    StoredFranklinOpsBlock,
};

/// Updates stored tree state
///
/// # Arguments
///
/// * `block_number` - current block number
/// * `account_updates` - accounts updates
/// * `connection_pool` - Database Connection Pool
///
pub fn update_tree_state(
    block_number: u32,
    account_updates: &[(u32, AccountUpdate)],
    connection_pool: ConnectionPool,
) -> Result<(), failure::Error> {
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore save tree state: {}",
            e.to_string()
        )
    })?;
    storage
        .update_tree_state(block_number, &account_updates)
        .map_err(|e| format_err!("Cant save tree state: {}", e.to_string()))?;
    Ok(())
}

/// Saves Franklin Contract events in storage
///
/// # Arguments
///
/// * `events` - Franklin Contract events descriptions
/// * `connection_pool` - Database Connection Pool
///
pub fn save_events_state(
    events: &[EventData],
    connection_pool: ConnectionPool,
) -> Result<(), failure::Error> {
    let mut new_events: Vec<NewBlockEvent> = vec![];
    for event in events {
        new_events.push(
            block_event_into_stored_block_event(event)
                .ok_or_else(|| format_err!("Cant perform bock event into stored"))?,
        );
    }
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore save events: {}",
            e.to_string()
        )
    })?;
    storage
        .save_events_state(new_events.as_slice())
        .map_err(|e| format_err!("Cant save events state: {}", e.to_string()))?;
    Ok(())
}

/// Get Optional new stored representation of the Franklin Contract event from itself
///
/// # Arguments
///
/// * `evnet` - Franklin Contract event description
///
pub fn block_event_into_stored_block_event(event: &EventData) -> Option<NewBlockEvent> {
    Some(NewBlockEvent {
        block_type: match event.block_type {
            EventType::Committed => "Committed".to_string(),
            EventType::Verified => "Verified".to_string(),
        },
        transaction_hash: event.transaction_hash.as_bytes().to_vec(),
        block_num: i64::from(event.block_num),
    })
}

/// Saves last watched Ethereum block number in storage
///
/// # Arguments
///
/// * `number` - Last watched Ethereum block number
/// * `connection_pool` - Database Connection Pool
///
pub fn save_last_watched_block_number(
    number: u64,
    connection_pool: ConnectionPool,
) -> Result<(), failure::Error> {
    let block_number = NewLastWatchedEthBlockNumber {
        block_number: number.to_string(),
    };
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore save block number: {}",
            e.to_string()
        )
    })?;
    storage
        .save_last_watched_block_number(&block_number)
        .map_err(|e| format_err!("Cant save last watched block number: {}", e.to_string()))?;
    Ok(())
}

/// Saves update storage state
///
/// # Arguments
///
/// * `state` - storage state update
/// * `connection_pool` - Database Connection Pool
///
pub fn save_storage_state(
    state: StorageUpdateState,
    connection_pool: ConnectionPool,
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
        .save_storage_state(&storage_state)
        .map_err(|e| format_err!("Cant save storage state: {}", e.to_string()))?;
    Ok(())
}

/// Saves franklin operations blocks in storage
///
/// # Arguments
///
/// * `blocks` - Franklin operations blocks
/// * `connection_pool` - Database Connection Pool
///
pub fn save_franklin_ops_blocks(
    blocks: &[FranklinOpsBlock],
    connection_pool: ConnectionPool,
) -> Result<(), failure::Error> {
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore save ops blocks: {}",
            e.to_string()
        )
    })?;
    for block in blocks {
        storage
            .save_franklin_ops_block(block.ops.as_slice(), block.block_num, block.fee_account)
            .map_err(|e| format_err!("Cant save franklin transaction: {}", e.to_string()))?;
    }
    Ok(())
}

/// Removes events state from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn remove_events_state(connection_pool: ConnectionPool) -> Result<(), failure::Error> {
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore remove events state: {}",
            e.to_string()
        )
    })?;
    storage
        .delete_events_state()
        .map_err(|e| format_err!("No events state to delete: {}", e.to_string()))?;
    Ok(())
}

/// Removes franklin operations from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn remove_franklin_ops(connection_pool: ConnectionPool) -> Result<(), failure::Error> {
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore remove franklin ops: {}",
            e.to_string()
        )
    })?;
    storage
        .delete_franklin_ops()
        .map_err(|e| format_err!("No franklin ops to delete: {}", e.to_string()))?;
    Ok(())
}

/// Removes tree state from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn remove_tree_state(connection_pool: ConnectionPool) -> Result<(), failure::Error> {
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore remove tree data: {}",
            e.to_string()
        )
    })?;
    storage
        .delete_tree_state()
        .map_err(|e| format_err!("No tree state to delete: {}", e.to_string()))?;
    Ok(())
}

/// Removes last watched block number from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn remove_last_watched_block_number(
    connection_pool: ConnectionPool,
) -> Result<(), failure::Error> {
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore remove last watched block number: {}",
            e.to_string()
        )
    })?;
    storage
        .delete_last_watched_block_number()
        .map_err(|e| format_err!("No last watched block number to delete: {}", e.to_string()))?;
    Ok(())
}

/// Removes update storage statae from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn remove_storage_state_status(connection_pool: ConnectionPool) -> Result<(), failure::Error> {
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore remove storage state status: {}",
            e.to_string()
        )
    })?;
    storage
        .delete_data_restore_storage_state_status()
        .map_err(|e| format_err!("No storage state status to delete: {}", e.to_string()))?;
    Ok(())
}

/// Get Franklin operations blocks from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_ops_blocks_from_storage(
    connection_pool: ConnectionPool,
) -> Result<Vec<FranklinOpsBlock>, failure::Error> {
    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for data restore get ops blocks: {}",
            e.to_string()
        )
    })?;
    let committed_blocks = storage
        .load_franklin_ops_blocks()
        .map_err(|e| format_err!("No ops blocks to delete: {}", e.to_string()))?;
    let mut blocks: Vec<FranklinOpsBlock> = vec![];
    for block in committed_blocks {
        blocks.push(stored_ops_block_into_ops_block(&block));
    }
    Ok(blocks)
}

/// Get Franklin Operations Block from its stored representation
///
/// # Arguments
///
/// * `op_block` - Stored Franklin operations block description
///
pub fn stored_ops_block_into_ops_block(op_block: &StoredFranklinOpsBlock) -> FranklinOpsBlock {
    FranklinOpsBlock {
        block_num: op_block.block_num,
        ops: op_block.ops.clone(),
        fee_account: op_block.fee_account,
    }
}

/// Get storage update state from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_storage_state(
    connection_pool: ConnectionPool,
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

/// Get last watched ethereum block number from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_last_watched_block_number_from_storage(
    connection_pool: ConnectionPool,
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

/// Get Events State from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_events_state_from_storage(
    connection_pool: ConnectionPool,
) -> Result<EventsState, failure::Error> {
    let last_watched_eth_block_number =
        get_last_watched_block_number_from_storage(connection_pool.clone())?;

    let storage = connection_pool.access_storage().map_err(|e| {
        format_err!(
            "Db connection failed for get past events: {}",
            e.to_string()
        )
    })?;

    let committed = storage
        .load_committed_events_state()
        .map_err(|e| format_err!("Load committed state failed: {}", e.to_string()))?;

    let mut committed_events: Vec<EventData> = vec![];
    for event in committed {
        let block_event = stored_block_event_into_block_event(event.clone())
            .ok_or_else(|| format_err!("Block events db is broken"))?;
        committed_events.push(block_event);
    }

    let verified = storage
        .load_verified_events_state()
        .map_err(|e| format_err!("Db connection failed for past events: {}", e.to_string()))?;
    let mut verified_events: Vec<EventData> = vec![];
    for event in verified {
        let block_event = stored_block_event_into_block_event(event.clone())
            .ok_or_else(|| format_err!("Block events db is broken"))?;
        verified_events.push(block_event);
    }

    Ok(EventsState {
        committed_events,
        verified_events,
        last_watched_eth_block_number,
    })
}

/// Get Optional Franklin Contract event from its stored representation
///
/// # Arguments
///
/// * `block` - Stored representation of Franklin Contract event
///
pub fn stored_block_event_into_block_event(block: StoredBlockEvent) -> Option<EventData> {
    Some(EventData {
        block_num: u32::try_from(block.block_num).ok()?,
        transaction_hash: H256::from_slice(block.transaction_hash.as_slice()),
        block_type: match &block.block_type {
            c if c == "Committed" => EventType::Committed,
            v if v == "Verified" => EventType::Verified,
            _ => return None,
        },
    })
}

/// Get tree accounts state and last block number from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_tree_state(
    connection_pool: ConnectionPool,
) -> Result<(u32, AccountMap), failure::Error> {
    let storage = connection_pool
        .access_storage()
        .map_err(|e| format_err!("Db connection failed for tree state: {}", e.to_string()))?;

    let tree_state = storage
        .load_tree_state()
        .map_err(|e| format_err!("get_tree_state: db must work: {}", e.to_string()))?;

    Ok(tree_state)
}
