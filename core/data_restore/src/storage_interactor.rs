use crate::events::{EventData, EventType};
use crate::events_state::EventsState;
use crate::franklin_op_block::{FranklinOpBlock, FranklinOpBlockType};
use crate::helpers;
use std::convert::TryFrom;
use storage::{
    ConnectionPool, NewBlockLog, NewFranklinOpBlock,
    NewLastWatchedEthBlockNumber, StoredBlockLog, StoredFranklinOpBlock,
};
use web3::types::{Bytes, Transaction, H160, H256, U128, U256};

/// Removes stored data
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn remove_storage_data(
    connection_pool: ConnectionPool,
) -> Result<(), helpers::DataRestoreError> {
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for data restore remove data");
    let delete_events_state_res = storage.delete_events_state();
    let delete_last_watched_block_number_res = storage.delete_last_watched_block_number();
    let delete_franklin_op_blocks_res = storage.delete_franklin_op_blocks();
    if delete_events_state_res.is_err() {
        return Err(helpers::DataRestoreError::NoData(
            "No block events in storage".to_string(),
        ));
    }
    if delete_last_watched_block_number_res.is_err() {
        return Err(helpers::DataRestoreError::NoData(
            "No block number in storage".to_string(),
        ));
    }
    if delete_franklin_op_blocks_res.is_err() {
        return Err(helpers::DataRestoreError::NoData(
            "No franklin txs in storage".to_string(),
        ));
    }
    Ok(())
}

/// Saves Franklin Contract events in storage
///
/// # Arguments
///
/// * `events` - Franklin Contract events descriptions
/// * `connection_pool` - Database Connection Pool
///
pub fn save_events_state(events: &Vec<EventData>, connection_pool: ConnectionPool) {
    let mut new_logs: Vec<NewBlockLog> = vec![];
    for log in events {
        new_logs
            .push(block_log_into_stored_block_log(log).expect("cant perform bock log into stored"));
    }
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for data restore save events");
    storage
        .save_events_state(new_logs.as_slice())
        .expect("cant save events state");
}

/// Saves last watched Ethereum block number in storage
///
/// # Arguments
///
/// * `number` - Last watched Ethereum block number
/// * `connection_pool` - Database Connection Pool
///
pub fn save_last_watched_block_number(number: &U256, connection_pool: ConnectionPool) {
    let block_number = NewLastWatchedEthBlockNumber {
        block_number: number.to_string(),
    };
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for data restore save block number");
    if let Err(_) = storage.delete_last_watched_block_number() {
        info!("First time saving last watched block number");
    }
    storage
        .save_last_watched_block_number(&block_number)
        .expect("cant save last watched block number");
}

/// Saves franklin operation blocks in storage
///
/// # Arguments
///
/// * `blocks` - Franklin operation blocks
/// * `connection_pool` - Database Connection Pool
///
pub fn save_franklin_op_blocks(blocks: &Vec<FranklinOpBlock>, connection_pool: ConnectionPool) {
    let mut stored_blocks: Vec<NewFranklinOpBlock> = vec![];
    for block in blocks {
        stored_blocks.push(op_block_into_stored_op_block(&block));
    }
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for data restore save transactions");
    storage
        .save_franklin_op_blocks(&stored_blocks.as_slice())
        .expect("cant save franklin transaction");
}

/// Get Optional Franklin Contract event from its stored representation
///
/// # Arguments
///
/// * `block` - Stored representation of Franklin Contract event
///
pub fn stored_block_log_into_block_log(block: &StoredBlockLog) -> Option<EventData> {
    Some(EventData {
        block_num: u32::try_from(block.block_num)
            .expect("cant make block_num in stored_block_log_into_block_log"),
        transaction_hash: H256::from_slice(block.transaction_hash.as_slice()),
        block_type: match &block.block_type {
            c if c == "Committed" => EventType::Committed,
            v if v == "Verified" => EventType::Verified,
            _ => return None,
        },
    })
}

/// Get Optional new stored representation of the Franklin Contract event from itself
///
/// # Arguments
///
/// * `block` - Franklin Contract event description
///
pub fn block_log_into_stored_block_log(block: &EventData) -> Option<NewBlockLog> {
    Some(NewBlockLog {
        block_type: match block.block_type {
            EventType::Committed => "Committed".to_string(),
            EventType::Verified => "Verified".to_string(),
            EventType::Unknown => return None,
        },
        transaction_hash: block.transaction_hash.as_bytes().to_vec(),
        block_num: i64::from(block.block_num),
    })
}

/// Get new stored represantation of the Franklin operations block from itself
///
/// # Arguments
///
/// * `op_block` - Franklin operations block description
///
pub fn op_block_into_stored_op_block(op_block: &FranklinOpBlock) -> NewFranklinOpBlock {
    let franklin_op_block_type = match op_block.franklin_op_block_type {
        FranklinOpBlockType::Deposit => format!("Deposit"),
        FranklinOpBlockType::Transfer => format!("Transfer"),
        FranklinOpBlockType::FullExit => format!("FullExit"),
        FranklinOpBlockType::Unknown => format!("Unknown"),
    };
    let block_number = i64::from(op_block.block_number);

    let eth_tx_hash = op_block.ethereum_transaction.hash.as_bytes().to_vec();
    let eth_tx_nonce = op_block.ethereum_transaction.nonce.to_string();
    let eth_tx_block_hash = op_block
        .ethereum_transaction
        .block_hash
        .map_or(None, |x| Some(x.as_bytes().to_vec()));
    let eth_tx_block_number = op_block
        .ethereum_transaction
        .block_number
        .map_or(None, |x| Some(x.to_string()));
    let eth_tx_transaction_index = op_block
        .ethereum_transaction
        .transaction_index
        .map_or(None, |x| Some(x.to_string()));
    let eth_tx_from = op_block.ethereum_transaction.from.as_bytes().to_vec();
    let eth_tx_to = op_block
        .ethereum_transaction
        .to
        .map_or(None, |x| Some(x.as_bytes().to_vec()));
    let eth_tx_value = op_block.ethereum_transaction.value.to_string();
    let eth_tx_gas_price = op_block.ethereum_transaction.gas_price.to_string();
    let eth_tx_gas = op_block.ethereum_transaction.gas.to_string();
    let eth_tx_input = op_block.ethereum_transaction.input.0.clone();

    let commitment_data = op_block.commitment_data.clone();

    NewFranklinOpBlock {
        franklin_op_block_type,
        block_number,
        eth_tx_hash,
        eth_tx_nonce,
        eth_tx_block_hash,
        eth_tx_block_number,
        eth_tx_transaction_index,
        eth_tx_from,
        eth_tx_to,
        eth_tx_value,
        eth_tx_gas_price,
        eth_tx_gas,
        eth_tx_input,
        commitment_data,
    }
}

/// Get Franklin Operations Block from its stored representation
///
/// # Arguments
///
/// * `op_block` - Stored Franklin operations block description
///
pub fn stored_op_block_into_op_block(op_block: &StoredFranklinOpBlock) -> FranklinOpBlock {
    let franklin_op_block_type = match op_block.franklin_op_block_type.as_str() {
        d if d == "Deposit" => FranklinOpBlockType::Deposit,
        t if t == "Transfer" => FranklinOpBlockType::Transfer,
        e if e == "FullExit" => FranklinOpBlockType::FullExit,
        _ => FranklinOpBlockType::Unknown,
    };
    let bn = u32::try_from(op_block.block_number)
        .expect("cant make bn in stored_op_block_into_op_block");
    let hash = H256::from_slice(op_block.eth_tx_hash.as_slice());
    let nonce = U256::from_dec_str(op_block.eth_tx_nonce.as_str())
        .expect("cant make nonce in stored_op_block_into_op_block");
    let block_hash = match &op_block.eth_tx_block_hash {
        None => None,
        Some(x) => Some(H256::from_slice(x.as_slice())),
    };
    let block_number = match &op_block.eth_tx_block_number {
        None => None,
        Some(x) => Some(
            U256::from_dec_str(x.as_str())
                .expect("cant make block_number in stored_op_block_into_op_block"),
        ),
    };
    let transaction_index = match &op_block.eth_tx_transaction_index {
        None => None,
        Some(x) => Some(
            U128::from_dec_str(x.as_str())
                .expect("cant make transaction_index in stored_op_block_into_op_block"),
        ),
    };
    let from = H160::from_slice(op_block.eth_tx_from.as_slice());
    let to = match &op_block.eth_tx_to {
        None => None,
        Some(x) => Some(H160::from_slice(x.as_slice())),
    };
    let value = U256::from_dec_str(op_block.eth_tx_value.as_str())
        .expect("cant make value in stored_op_block_into_op_block");
    let gas_price = U256::from_dec_str(op_block.eth_tx_gas_price.as_str())
        .expect("cant make gas_price in stored_op_block_into_op_block");
    let gas = U256::from_dec_str(op_block.eth_tx_gas.as_str())
        .expect("cant make gas in stored_op_block_into_op_block");
    let input = Bytes(op_block.eth_tx_input.clone());
    let commitment_data = op_block.commitment_data.clone();

    let ethereum_transaction = Transaction {
        hash,
        nonce,
        block_hash,
        block_number,
        transaction_index,
        from,
        to,
        value,
        gas_price,
        gas,
        input,
    };

    FranklinOpBlock {
        franklin_op_block_type,
        block_number: bn,
        ethereum_transaction,
        commitment_data,
    }
}

/// Get Franklin operations blocks from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_op_blocks_from_storage(connection_pool: ConnectionPool) -> Vec<FranklinOpBlock> {
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for past events");
    let committed_blocks = storage.load_franklin_op_blocks();
    let mut blocks: Vec<FranklinOpBlock> = vec![];
    for block in committed_blocks {
        blocks.push(stored_op_block_into_op_block(&block));
    }
    blocks
}

/// Get last watched ethereum block number from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_last_watched_block_number_from_storage(connection_pool: ConnectionPool) -> U256 {
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for past events");

    let last_watched_block_number_string = storage
        .load_last_watched_block_number()
        .expect("load_blocks_events: db must work")
        .block_number;
    U256::from_dec_str(last_watched_block_number_string.as_str())
        .expect("cant make u256 block_number in get_last_watched_block_number_from_storage")
}

/// Get Events State from storage
///
/// # Arguments
///
/// * `connection_pool` - Database Connection Pool
///
pub fn get_events_state_from_storage(connection_pool: ConnectionPool, config: helpers::DataRestoreConfig) -> EventsState {
    let last_watched_block_number =
        get_last_watched_block_number_from_storage(connection_pool.clone());

    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for past events");

    let committed_logs = storage.load_committed_events_state();
    let mut committed_blocks: Vec<EventData> = vec![];
    for log in committed_logs {
        let block_log = stored_block_log_into_block_log(&log).expect("block logs db is broken");
        committed_blocks.push(block_log);
    }

    let verified_logs = storage.load_verified_events_state();
    let mut verified_blocks: Vec<EventData> = vec![];
    for log in verified_logs {
        let block_log = stored_block_log_into_block_log(&log).expect("block logs db is broken");
        verified_blocks.push(block_log);
    }

    EventsState {
        config,
        committed_blocks,
        verified_blocks,
        last_watched_block_number,
    }
}
