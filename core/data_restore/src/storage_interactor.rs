use std::str::FromStr;
use storage::StoredBlockLog;
use crate::franklin_transaction::{FranklinTransactionType, FranklinTransaction};
use storage::ConnectionPool;
use web3::types::{U256, H256, H160, U128, Transaction, Bytes};
use crate::blocks::{BlockType, LogBlockData};
use crate::block_events::BlockEventsFranklin;
use crate::helpers;

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

pub fn stored_block_log_into_block_log(block: &StoredBlockLog) -> Option<LogBlockData> {
    let mut block_log = LogBlockData {
        block_num: block.block_num as u32,
        transaction_hash: H256::from_str(block.transaction_hash.as_str()).unwrap(),
        block_type: BlockType::Unknown,
    };
    block_log.block_type = match &block.block_type {
        c if c == "Committed" => BlockType::Committed,
        v if v == "Verified" => BlockType::Verified,
        _ => return None,
    };
    Some(block_log)
}

pub fn load_config_from_storage(connection_pool: ConnectionPool) -> Option<helpers::DataRestoreConfig> {
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

pub fn get_transactions_from_storage(connection_pool: ConnectionPool) -> Vec<FranklinTransaction> {
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for past events");
    let committed_txs = storage
        .load_franklin_transactions();
    let mut txs: Vec<FranklinTransaction> = vec![];
    for tx in committed_txs {
        let franklin_transaction_type = match tx.franklin_transaction_type.as_str() {
            d if d == "Deposit" => FranklinTransactionType::Deposit,
            t if t == "Transfer" => FranklinTransactionType::Transfer,
            e if e == "FullExit" => FranklinTransactionType::FullExit,
            _ => FranklinTransactionType::Unknown,
        };
        let bn = tx.block_number as u32;
        let hash = H256::from_str(tx.eth_tx_hash.as_str()).unwrap();
        let nonce = U256::from_str(tx.eth_tx_nonce.as_str()).unwrap();
        let block_hash = match tx.eth_tx_block_hash {
            None => None,
            Some(x) => Some(
                H256::from_str(x.as_str()).unwrap()
            ),
        };
        let block_number = match tx.eth_tx_block_number {
            None => None,
            Some(x) => Some(
                U256::from_str(x.as_str()).unwrap()
            ),
        };
        let transaction_index = match tx.eth_tx_transaction_index {
            None => None,
            Some(x) => Some(
                U128::from_str(x.as_str()).unwrap()
            ),
        };
        let from = H160::from_str(tx.eth_tx_from.as_str()).unwrap();
        let to = match tx.eth_tx_to {
            None => None,
            Some(x) => Some(
                H160::from_str(x.as_str()).unwrap()
            ),
        };
        let value = U256::from_str(tx.eth_tx_value.as_str()).unwrap();
        let gas_price = U256::from_str(tx.eth_tx_gas_price.as_str()).unwrap();
        let gas = U256::from_str(tx.eth_tx_gas.as_str()).unwrap();
        let input = Bytes(tx.eth_tx_input);
        let commitment_data = tx.commitment_data;

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

        let f_tx = FranklinTransaction {
            franklin_transaction_type,
            block_number: bn,
            ethereum_transaction,
            commitment_data,
        };

        txs.push(f_tx);
    }
    txs
}

pub fn get_block_events_from_storage(config: helpers::DataRestoreConfig, connection_pool: ConnectionPool) -> BlockEventsFranklin {
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for past events");
    
    let committed_logs = storage
        .load_committed_block_events();
    let mut committed_blocks: Vec<LogBlockData> = vec![];
    for log in committed_logs {
        let block_log = stored_block_log_into_block_log(&log).expect("block logs db is broken");
        committed_blocks.push(block_log);
    }

    let verified_logs = storage
        .load_verified_block_events();
    let mut verified_blocks: Vec<LogBlockData> = vec![];
    for log in verified_logs {
        let block_log = stored_block_log_into_block_log(&log).expect("block logs db is broken");
        verified_blocks.push(block_log);
    }

    let last_watched_block_number_string = storage
        .load_last_watched_block_number()
        .expect("load_blocks_events: db must work")
        .number;
    let last_watched_block_number = U256::from_str(last_watched_block_number_string.as_str()).unwrap();
    
    BlockEventsFranklin {
        config,
        committed_blocks,
        verified_blocks,
        last_watched_block_number,
    }
}
