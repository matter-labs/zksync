use crate::events::{EventData, EventType};
use crate::events_state::EventsState;
use crate::franklin_ops::FranklinOpsBlock;
use crate::helpers::DataRestoreError;
use std::convert::TryFrom;
use storage::{
    ConnectionPool, NewBlockEvent, StoredBlockEvent, NewFranklinOp, StoredFranklinOp,
    NewLastWatchedEthBlockNumber, StoredLastWatchedEthBlockNumber, StoredFranklinOpsBlock,
};
use models::node::operations::FranklinOp;
use models::node::{AccountUpdates, AccountMap};
use web3::types::{Bytes, Transaction, H160, H256, U128, U256};

/// Removes stored data
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn remove_storage_data(connection_pool: ConnectionPool) -> Result<(), DataRestoreError> {
    remove_last_watched_block_number(connection_pool.clone())?;
    remove_events_state(connection_pool.clone())?;
    remove_franklin_ops(connection_pool.clone())?;
    remove_tree_state(connection_pool.clone())?;
    Ok(())
}


pub fn update_tree_state(block_number: u32, account_updates: &AccountUpdates, connection_pool: ConnectionPool) -> Result<(), DataRestoreError> {
    let storage = connection_pool
        .access_storage()
        .map_err(|_| DataRestoreError::Storage("db connection failed for data restore save tree state".to_string()))?;
    storage
        .update_tree_state(block_number, &account_updates)
        .map_err(|_| DataRestoreError::Storage("cant save tree state".to_string()))?;
    Ok(())
}

/// Saves Franklin Contract events in storage
///
/// # Arguments
///
/// * `events` - Franklin Contract events descriptions
/// * `connection_pool` - Database Connection Pool
///
pub fn save_events_state(events: &Vec<EventData>, connection_pool: ConnectionPool) -> Result<(), DataRestoreError> {
    let mut new_events: Vec<NewBlockEvent> = vec![];
    for event in events {
        new_events
            .push(
                block_event_into_stored_block_event(event)
                    .ok_or(DataRestoreError::Storage("cant perform bock event into stored".to_string()))?
            );
    }
    let storage = connection_pool
        .access_storage()
        .map_err(|_| DataRestoreError::Storage("db connection failed for data restore save events".to_string()))?;
    storage
        .save_events_state(new_events.as_slice())
        .map_err(|_| DataRestoreError::Storage("cant save events state".to_string()))?;
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
            EventType::Unknown => return None,
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
pub fn save_last_watched_block_number(number: &u64, connection_pool: ConnectionPool) -> Result<(), DataRestoreError> {
    let block_number = NewLastWatchedEthBlockNumber {
        block_number: number.to_string(),
    };
    let storage = connection_pool
        .access_storage()
        .map_err(|_| DataRestoreError::Storage("db connection failed for data restore save block number".to_string()))?;
    storage
        .save_last_watched_block_number(&block_number)
        .map_err(|_| DataRestoreError::Storage("cant save last watched block number".to_string()))?;
    Ok(())
}

/// Saves franklin operation blocks in storage
///
/// # Arguments
///
/// * `blocks` - Franklin operation blocks
/// * `connection_pool` - Database Connection Pool
///
pub fn save_franklin_ops_blocks(blocks: &Vec<FranklinOpsBlock>, connection_pool: ConnectionPool) -> Result<(), DataRestoreError> {
    let storage = connection_pool
        .access_storage()
        .map_err(|_| DataRestoreError::Storage("db connection failed for data restore save transactions".to_string()))?;
    for block in blocks {
        storage
            .save_franklin_ops_block(block.ops.as_slice(), block.block_num)
            .map_err(|_| DataRestoreError::Storage("cant save franklin transaction".to_string()))?;
    }
    Ok(())
}

/// Data removes
/// 
pub fn remove_events_state(connection_pool: ConnectionPool) -> Result<(), DataRestoreError> {
    let storage = connection_pool
        .access_storage()
        .map_err(|_| DataRestoreError::Storage("db connection failed for data restore remove data".to_string()))?;
    let delete_events_state_res = storage.delete_events_state()
        .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
    Ok(())
}

pub fn remove_franklin_ops(connection_pool: ConnectionPool) -> Result<(), DataRestoreError> {
    let storage = connection_pool
        .access_storage()
        .map_err(|_| DataRestoreError::Storage("db connection failed for data restore remove data".to_string()))?;
    let delete_franklin_ops_res = storage.delete_franklin_ops()
        .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
    Ok(())
}

pub fn remove_tree_state(connection_pool: ConnectionPool) -> Result<(), DataRestoreError> {
    let storage = connection_pool
        .access_storage()
        .map_err(|_| DataRestoreError::Storage("db connection failed for data restore remove data".to_string()))?;
    let delete_tree_state_res = storage.delete_tree_state()
        .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
    Ok(())
}

pub fn remove_last_watched_block_number(connection_pool: ConnectionPool) -> Result<(), DataRestoreError> {
    let storage = connection_pool
        .access_storage()
        .map_err(|_| DataRestoreError::Storage("db connection failed for data restore remove data".to_string()))?;
    let delete_last_watched_block_number_res = storage.delete_last_watched_block_number()
        .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
    Ok(())
}

/// Get Franklin operations blocks from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_ops_blocks_from_storage(connection_pool: ConnectionPool) -> Result<Vec<FranklinOpsBlock>, DataRestoreError> {
    let storage = connection_pool
        .access_storage()
        .map_err(|_| DataRestoreError::Storage("db connection failed for past events".to_string()))?;
    let committed_blocks = storage.load_franklin_ops_blocks()
        .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
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
        ops: op_block.ops.clone()
    }
} 

/// Get last watched ethereum block number from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_last_watched_block_number_from_storage(connection_pool: ConnectionPool) -> Result<u64, DataRestoreError> {
    let storage = connection_pool
        .access_storage()
        .map_err(|_| DataRestoreError::Storage("db connection failed for block number".to_string()))?;

    let last_watched_block_number_string = storage
        .load_last_watched_block_number()
        .map_err(|_| DataRestoreError::Storage("load_blocks_events: db must work".to_string()))?
        .block_number;

    Ok(u64::from_str_radix(last_watched_block_number_string.as_str(), 10)
        .map_err(|_| DataRestoreError::Unknown("cant make u256 block_number in get_last_watched_block_number_from_storage".to_string()))?)
}

/// Get Events State from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_events_state_from_storage(connection_pool: ConnectionPool) -> Result<EventsState, DataRestoreError> {
    let last_watched_eth_block_number =
        get_last_watched_block_number_from_storage(connection_pool.clone())?;

    let storage = connection_pool
        .access_storage()
        .map_err(|_| DataRestoreError::Storage("db connection failed for past events".to_string()))?;

    let committed = storage.load_committed_events_state()
        .map_err(|_| DataRestoreError::Storage("db connection failed for past events".to_string()))?;
    let mut committed_events: Vec<EventData> = vec![];
    for event in committed {
        let block_event = stored_block_event_into_block_event(event.clone())
            .ok_or(DataRestoreError::Unknown("block events db is broken".to_string()))?;
        committed_events.push(block_event);
    }

    let verified = storage.load_verified_events_state()
        .map_err(|_| DataRestoreError::Storage("db connection failed for past events".to_string()))?;
    let mut verified_events: Vec<EventData> = vec![];
    for event in verified {
        let block_event = stored_block_event_into_block_event(event.clone())
            .ok_or(DataRestoreError::Unknown("block events db is broken".to_string()))?;
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
        block_num: u32::try_from(block.block_num)
            .ok()?,
        transaction_hash: H256::from_slice(block.transaction_hash.as_slice()),
        block_type: match &block.block_type {
            c if c == "Committed" => EventType::Committed,
            v if v == "Verified" => EventType::Verified,
            _ => return None,
        },
    })
}

pub fn get_tree_state(connection_pool: ConnectionPool) -> Result<(u32, AccountMap), DataRestoreError> {
    let storage = connection_pool
        .access_storage()
        .map_err(|_| DataRestoreError::Storage("db connection failed for tree state".to_string()))?;

    let tree_state = storage
        .load_tree_state()
        .map_err(|_| DataRestoreError::Storage("load_tree_state: db must work".to_string()))?;

    Ok(tree_state)
}
