use crate::events_state::EventsState;
use crate::events::{EventType, EventData};
use crate::franklin_transaction::{FranklinTransaction, FranklinTransactionType};
use crate::helpers;
use std::convert::TryFrom;
use storage::{
    ConnectionPool, NewBlockLog, NewFranklinTransaction, NewLastWatchedEthBlockNumber,
    NewTreeRestoreNetwork, StoredBlockLog, StoredFranklinTransaction,
};
use web3::types::{Bytes, Transaction, H160, H256, U128, U256};

pub fn remove_storage_data(
    connection_pool: ConnectionPool,
) -> Result<(), helpers::DataRestoreError> {
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for tree restore remove data");
    let delete_tree_restore_network_res = storage.delete_tree_restore_network();
    let delete_events_state_res = storage.delete_events_state();
    let delete_last_watched_block_number_res = storage.delete_last_watched_block_number();
    let delete_franklin_transactions_res = storage.delete_franklin_transactions();
    if delete_tree_restore_network_res.is_err() {
        return Err(helpers::DataRestoreError::NoData(
            "No network in storage".to_string(),
        ));
    }
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
    if delete_franklin_transactions_res.is_err() {
        return Err(helpers::DataRestoreError::NoData(
            "No franklin txs in storage".to_string(),
        ));
    }
    Ok(())
}

pub fn save_tree_restore_from_config(
    config: &helpers::DataRestoreConfig,
    connection_pool: ConnectionPool,
) {
    let network = NewTreeRestoreNetwork {
        network_id: config.network_id,
    };
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for tree restore save network");
    if let Err(_) = storage.delete_tree_restore_network() {
        info!("First time saving tree restore network");
    }
    storage
        .save_tree_restore_network(&network)
        .expect("cant save tree restore network");
}

pub fn save_events_state(events: &Vec<EventData>, connection_pool: ConnectionPool) {
    let mut new_logs: Vec<NewBlockLog> = vec![];
    for log in events {
        new_logs
            .push(block_log_into_stored_block_log(log).expect("cant perform bock log into stored"));
    }
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for tree restore save events");
    storage
        .save_events_state(new_logs.as_slice())
        .expect("cant save tree restore network");
}

pub fn save_last_watched_block_number(number: &U256, connection_pool: ConnectionPool) {
    let block_number = NewLastWatchedEthBlockNumber {
        block_number: number.to_string(),
    };
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for tree restore save block number");
    if let Err(_) = storage.delete_last_watched_block_number() {
        info!("First time saving last watched block number");
    }
    storage
        .save_last_watched_block_number(&block_number)
        .expect("cant save last watched block number");
}

pub fn save_franklin_transactions(txs: &Vec<FranklinTransaction>, connection_pool: ConnectionPool) {
    let mut stored_txs: Vec<NewFranklinTransaction> = vec![];
    for tx in txs {
        stored_txs.push(transaction_into_stored_transaction(&tx));
    }
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for tree restore save transactions");
    storage
        .save_franklin_transactions(&stored_txs.as_slice())
        .expect("cant save franklin transaction");
}

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

pub fn get_config_from_storage(
    connection_pool: ConnectionPool,
) -> Option<helpers::DataRestoreConfig> {
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for tree restore config");
    let network_id = storage
        .load_tree_restore_network()
        .expect("can not load network")
        .network_id;
    match network_id {
        1 => Some(helpers::DataRestoreConfig::new(
            helpers::InfuraEndpoint::Mainnet,
        )),
        4 => Some(helpers::DataRestoreConfig::new(
            helpers::InfuraEndpoint::Rinkeby,
        )),
        _ => None,
    }
}

pub fn transaction_into_stored_transaction(tx: &FranklinTransaction) -> NewFranklinTransaction {
    let franklin_transaction_type = match tx.franklin_transaction_type {
        FranklinTransactionType::Deposit => format!("Deposit"),
        FranklinTransactionType::Transfer => format!("Transfer"),
        FranklinTransactionType::FullExit => format!("FullExit"),
        FranklinTransactionType::Unknown => format!("Unknown"),
    };
    let block_number = i64::from(tx.block_number);

    let eth_tx_hash = tx.ethereum_transaction.hash.as_bytes().to_vec();
    let eth_tx_nonce = tx.ethereum_transaction.nonce.to_string();
    let eth_tx_block_hash = tx
        .ethereum_transaction
        .block_hash
        .map_or(None, |x| Some(x.as_bytes().to_vec()));
    let eth_tx_block_number = tx
        .ethereum_transaction
        .block_number
        .map_or(None, |x| Some(x.to_string()));
    let eth_tx_transaction_index = tx
        .ethereum_transaction
        .transaction_index
        .map_or(None, |x| Some(x.to_string()));
    let eth_tx_from = tx.ethereum_transaction.from.as_bytes().to_vec();
    let eth_tx_to = tx
        .ethereum_transaction
        .to
        .map_or(None, |x| Some(x.as_bytes().to_vec()));
    let eth_tx_value = tx.ethereum_transaction.value.to_string();
    let eth_tx_gas_price = tx.ethereum_transaction.gas_price.to_string();
    let eth_tx_gas = tx.ethereum_transaction.gas.to_string();
    let eth_tx_input = tx.ethereum_transaction.input.0.clone();

    let commitment_data = tx.commitment_data.clone();

    NewFranklinTransaction {
        franklin_transaction_type,
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

pub fn stored_transaction_into_transaction(tx: &StoredFranklinTransaction) -> FranklinTransaction {
    let franklin_transaction_type = match tx.franklin_transaction_type.as_str() {
        d if d == "Deposit" => FranklinTransactionType::Deposit,
        t if t == "Transfer" => FranklinTransactionType::Transfer,
        e if e == "FullExit" => FranklinTransactionType::FullExit,
        _ => FranklinTransactionType::Unknown,
    };
    let bn = u32::try_from(tx.block_number)
        .expect("cant make bn in stored_transaction_into_transaction");
    let hash = H256::from_slice(tx.eth_tx_hash.as_slice());
    let nonce = U256::from_dec_str(tx.eth_tx_nonce.as_str())
        .expect("cant make nonce in stored_transaction_into_transaction");
    let block_hash = match &tx.eth_tx_block_hash {
        None => None,
        Some(x) => Some(H256::from_slice(x.as_slice())),
    };
    let block_number = match &tx.eth_tx_block_number {
        None => None,
        Some(x) => Some(
            U256::from_dec_str(x.as_str())
                .expect("cant make block_number in stored_transaction_into_transaction"),
        ),
    };
    let transaction_index = match &tx.eth_tx_transaction_index {
        None => None,
        Some(x) => Some(
            U128::from_dec_str(x.as_str())
                .expect("cant make transaction_index in stored_transaction_into_transaction"),
        ),
    };
    let from = H160::from_slice(tx.eth_tx_from.as_slice());
    let to = match &tx.eth_tx_to {
        None => None,
        Some(x) => Some(H160::from_slice(x.as_slice())),
    };
    let value = U256::from_dec_str(tx.eth_tx_value.as_str())
        .expect("cant make value in stored_transaction_into_transaction");
    let gas_price = U256::from_dec_str(tx.eth_tx_gas_price.as_str())
        .expect("cant make gas_price in stored_transaction_into_transaction");
    let gas = U256::from_dec_str(tx.eth_tx_gas.as_str())
        .expect("cant make gas in stored_transaction_into_transaction");
    let input = Bytes(tx.eth_tx_input.clone());
    let commitment_data = tx.commitment_data.clone();

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

    FranklinTransaction {
        franklin_transaction_type,
        block_number: bn,
        ethereum_transaction,
        commitment_data,
    }
}

pub fn get_transactions_from_storage(connection_pool: ConnectionPool) -> Vec<FranklinTransaction> {
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for past events");
    let committed_txs = storage.load_franklin_transactions();
    let mut txs: Vec<FranklinTransaction> = vec![];
    for tx in committed_txs {
        txs.push(stored_transaction_into_transaction(&tx));
    }
    txs
}

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

pub fn get_events_state_from_storage(connection_pool: ConnectionPool) -> EventsState {
    let config = get_config_from_storage(connection_pool.clone())
        .expect("cant get config from storage in get_events_state_from_storage");
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
