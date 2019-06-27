use crate::block_events::BlockEventsFranklin;
use crate::blocks::{BlockType, LogBlockData};
use crate::franklin_transaction::{FranklinTransaction, FranklinTransactionType};
use crate::helpers;
use std::convert::TryFrom;
use std::str::FromStr;
use storage::{
    ConnectionPool, LastWatchedEthBlockNumber, StoredBlockLog, StoredFranklinTransaction,
    TreeRestoreNetwork,
};
use web3::types::{Bytes, Transaction, H160, H256, U128, U256};

pub fn remove_storage_data(connection_pool: ConnectionPool) {
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for tree restore remove data");
    storage
        .delete_tree_restore_network()
        .expect("error in deleting network data");
    storage
        .delete_block_events()
        .expect("error in deleting network data");
    storage
        .delete_last_watched_block_number()
        .expect("error in deleting network data");
    storage
        .delete_franklin_transactions()
        .expect("error in deleting network data");
}

pub fn save_tree_restore_from_config(
    config: &helpers::DataRestoreConfig,
    connection_pool: ConnectionPool,
) {
    let network = TreeRestoreNetwork {
        id: config.network_id,
    };
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for tree restore save network");
    storage.save_tree_restore_network(&network).unwrap();
}

pub fn save_block_events(events: &Vec<LogBlockData>, connection_pool: ConnectionPool) {
    let mut stored_logs: Vec<StoredBlockLog> = vec![];
    for log in events {
        stored_logs.push(block_log_into_stored_block_log(log).unwrap());
    }
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for tree restore save events");
    storage.save_block_events(stored_logs.as_slice()).unwrap();
}

pub fn save_last_watched_block_number(number: &U256, connection_pool: ConnectionPool) {
    let block_number = LastWatchedEthBlockNumber {
        number: number.to_string(),
    };
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for tree restore save block number");
    storage
        .save_last_watched_block_number(&block_number)
        .unwrap();
}

pub fn save_franklin_transactions(txs: &Vec<FranklinTransaction>, connection_pool: ConnectionPool) {
    let mut stored_txs: Vec<StoredFranklinTransaction> = vec![];
    for tx in txs {
        stored_txs.push(transaction_into_stored_transaction(&tx));
    }
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for tree restore save transactions");
    storage
        .save_franklin_transactions(&stored_txs.as_slice())
        .unwrap();
}

pub fn stored_block_log_into_block_log(block: &StoredBlockLog) -> Option<LogBlockData> {
    Some(LogBlockData {
        block_num: u32::try_from(block.block_num).unwrap(),
        transaction_hash: H256::from_str(block.transaction_hash.as_str()).unwrap(),
        block_type: match &block.block_type {
            c if c == "Committed" => BlockType::Committed,
            v if v == "Verified" => BlockType::Verified,
            _ => return None,
        },
    })
}

pub fn block_log_into_stored_block_log(block: &LogBlockData) -> Option<StoredBlockLog> {
    Some(StoredBlockLog {
        block_type: match block.block_type {
            BlockType::Committed => "Committed".to_string(),
            BlockType::Verified => "Verified".to_string(),
            BlockType::Unknown => return None,
        },
        transaction_hash: block.transaction_hash.to_string(),
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
        .id;
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

pub fn transaction_into_stored_transaction(tx: &FranklinTransaction) -> StoredFranklinTransaction {
    let franklin_transaction_type = match tx.franklin_transaction_type {
        FranklinTransactionType::Deposit => format!("Deposit"),
        FranklinTransactionType::Transfer => format!("Transfer"),
        FranklinTransactionType::FullExit => format!("FullExit"),
        FranklinTransactionType::Unknown => format!("Unknown"),
    };
    let block_number = i64::from(tx.block_number);

    let eth_tx_hash = tx.ethereum_transaction.hash.to_string();
    let eth_tx_nonce = tx.ethereum_transaction.nonce.to_string();
    let eth_tx_block_hash = tx
        .ethereum_transaction
        .block_hash
        .map_or(None, |x| Some(x.to_string()));
    let eth_tx_block_number = tx
        .ethereum_transaction
        .block_number
        .map_or(None, |x| Some(x.to_string()));
    let eth_tx_transaction_index = tx
        .ethereum_transaction
        .transaction_index
        .map_or(None, |x| Some(x.to_string()));
    let eth_tx_from = tx.ethereum_transaction.from.to_string();
    let eth_tx_to = tx
        .ethereum_transaction
        .to
        .map_or(None, |x| Some(x.to_string()));
    let eth_tx_value = tx.ethereum_transaction.value.to_string();
    let eth_tx_gas_price = tx.ethereum_transaction.gas_price.to_string();
    let eth_tx_gas = tx.ethereum_transaction.gas.to_string();
    let eth_tx_input = tx.ethereum_transaction.input.0.clone();

    let commitment_data = tx.commitment_data.clone();

    StoredFranklinTransaction {
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
    let bn = u32::try_from(tx.block_number).unwrap();
    let hash = H256::from_str(tx.eth_tx_hash.as_str()).unwrap();
    let nonce = U256::from_str(tx.eth_tx_nonce.as_str()).unwrap();
    let block_hash = match &tx.eth_tx_block_hash {
        None => None,
        Some(x) => Some(H256::from_str(x.as_str()).unwrap()),
    };
    let block_number = match &tx.eth_tx_block_number {
        None => None,
        Some(x) => Some(U256::from_str(x.as_str()).unwrap()),
    };
    let transaction_index = match &tx.eth_tx_transaction_index {
        None => None,
        Some(x) => Some(U128::from_str(x.as_str()).unwrap()),
    };
    let from = H160::from_str(tx.eth_tx_from.as_str()).unwrap();
    let to = match &tx.eth_tx_to {
        None => None,
        Some(x) => Some(H160::from_str(x.as_str()).unwrap()),
    };
    let value = U256::from_str(tx.eth_tx_value.as_str()).unwrap();
    let gas_price = U256::from_str(tx.eth_tx_gas_price.as_str()).unwrap();
    let gas = U256::from_str(tx.eth_tx_gas.as_str()).unwrap();
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
        .number;
    U256::from_str(last_watched_block_number_string.as_str()).unwrap()
}

pub fn get_block_events_from_storage(connection_pool: ConnectionPool) -> BlockEventsFranklin {
    let config = get_config_from_storage(connection_pool.clone()).unwrap();
    let last_watched_block_number =
        get_last_watched_block_number_from_storage(connection_pool.clone());

    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for past events");

    let committed_logs = storage.load_committed_block_events();
    let mut committed_blocks: Vec<LogBlockData> = vec![];
    for log in committed_logs {
        let block_log = stored_block_log_into_block_log(&log).expect("block logs db is broken");
        committed_blocks.push(block_log);
    }

    let verified_logs = storage.load_verified_block_events();
    let mut verified_blocks: Vec<LogBlockData> = vec![];
    for log in verified_logs {
        let block_log = stored_block_log_into_block_log(&log).expect("block logs db is broken");
        verified_blocks.push(block_log);
    }

    BlockEventsFranklin {
        config,
        committed_blocks,
        verified_blocks,
        last_watched_block_number,
    }
}
