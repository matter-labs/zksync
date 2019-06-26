#[macro_use]
extern crate log;

pub mod accounts_state;
pub mod block_events;
pub mod blocks;
pub mod data_restore_driver;
pub mod franklin_transaction;
pub mod helpers;

use crate::data_restore_driver::DataRestoreDriver;
use std::env;
use std::str::FromStr;
use storage::StoredBlockLog;
use franklin_transaction::{FranklinTransactionType, FranklinTransaction};
use storage::ConnectionPool;
use web3::types::{U256, H256, H160, U128, Transaction, Bytes};
use blocks::{BlockType, LogBlockData};
use block_events::BlockEventsFranklin;

fn create_new_data_restore_driver(
    config: helpers::DataRestoreConfig,
    from: U256,
    delta: U256,
) -> DataRestoreDriver {
    DataRestoreDriver::new(config, from, delta)
}

fn load_past_state_for_data_restore_driver(driver: &mut DataRestoreDriver, pool: ConnectionPool) {
    driver.load_past_state().expect("Cant get past state");
}

fn load_new_states_for_data_restore_driver(driver: &mut DataRestoreDriver, pool: ConnectionPool) {
    driver.run_state_updates().expect("Cant update state");
}

// pub fn load_new_states_for_stored_data_restore_driver() {
//     std::thread::Builder::new()
//         .name("data_restore".to_string())
//         .spawn(move || {
//             driver.run_state_updates().expect("Cant update state");
//         })
//         .expect("Load new states for data restore thread");
// }

// pub fn start_data_restore_driver(driver: &'static mut DataRestoreDriver) {
//     std::thread::Builder::new()
//         .name("data_restore".to_string())
//         .spawn(move || {
//             driver.load_past_state().expect("Cant get past state");
//             driver.run_state_updates().expect("Cant update state");
//         })
//         .expect("Data restore driver thread");
// }

fn load_states_from_beginning(args: Vec<String>) {
    let infura_endpoint_id =
        u8::from_str(&args[1]).expect("Network endpoint should be convertible to u8");
    info!("Network number is {}", &infura_endpoint_id);
    let config = match infura_endpoint_id {
        1 => Some(helpers::DataRestoreConfig::new(
            helpers::InfuraEndpoint::Mainnet,
        )),
        4 => Some(helpers::DataRestoreConfig::new(
            helpers::InfuraEndpoint::Rinkeby,
        )),
        _ => None,
    }
    .expect("It's acceptable only 1 for Mainnet and 4 for Rinkeby networks");
    let from = U256::from(0); // It's better not to allow external users to set "from block" parameter. In 99% cases 0(zero) is correct
    let delta = U256::from_str(&args[2]).expect("Blocks delta should be convertible to u256");
    info!("Blocks delta is {}", &delta);

    let connection_pool = ConnectionPool::new();

    let mut data_restore_driver = create_new_data_restore_driver(config, from, delta);
    load_past_state_for_data_restore_driver(&mut data_restore_driver, connection_pool.clone());
    load_new_states_for_data_restore_driver(&mut data_restore_driver, connection_pool.clone());
}

fn load_states_from_storage(args: Vec<String>) {
    let connection_pool = ConnectionPool::new();

    let config = load_config_from_storage(connection_pool.clone()).expect("Network id is broken in storage");
    let from = U256::from(0); // It's better not to allow external users to set "from block" parameter. In 99% cases 0(zero) is correct
    let delta = U256::from_str(&args[2]).expect("Blocks delta should be convertible to u256");
    info!("Blocks delta is {}", &delta);

    let mut data_restore_driver = create_new_data_restore_driver(config, from, delta);
    load_past_state_from_storage(&mut data_restore_driver, connection_pool.clone());
    load_new_states_for_data_restore_driver(&mut data_restore_driver, connection_pool.clone());
}

fn load_config_from_storage(connection_pool: ConnectionPool) -> Option<helpers::DataRestoreConfig> {
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

fn load_past_state_from_storage(driver: &mut DataRestoreDriver, connection_pool: ConnectionPool) {
    driver.block_events = get_block_events_from_storage(driver.config.clone(), connection_pool.clone());
    let transactions = get_transactions_from_storage(connection_pool.clone());
    for tx in transactions {
        driver.account_states.update_accounts_states_from_transaction(&tx)
            .expect("Cant update accounts state");
    }
}

fn get_transactions_from_storage(connection_pool: ConnectionPool) -> Vec<FranklinTransaction> {
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

pub fn into_block_log(block: &StoredBlockLog) -> Option<LogBlockData> {
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

fn get_block_events_from_storage(config: helpers::DataRestoreConfig, connection_pool: ConnectionPool) -> BlockEventsFranklin {
    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for past events");
    
    let committed_logs = storage
        .load_committed_block_events();
    let mut committed_blocks: Vec<LogBlockData> = vec![];
    for log in committed_logs {
        let block_log = into_block_log(&log).expect("block logs db is broken");
        committed_blocks.push(block_log);
    }

    let verified_logs = storage
        .load_verified_block_events();
    let mut verified_blocks: Vec<LogBlockData> = vec![];
    for log in verified_logs {
        let block_log = into_block_log(&log).expect("block logs db is broken");
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

fn main() {
    env_logger::init();
    info!("Hello, lets build Franklin accounts state");

    let args: Vec<String> = env::args().collect();

    if args[1] == "storage" {
        load_states_from_beginning(args);
    } else {
        load_states_from_storage(args);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_complete_task() {
        let config = helpers::DataRestoreConfig::new(helpers::InfuraEndpoint::Rinkeby);
        let from = U256::from(0);
        let delta = U256::from(15);
        let mut data_restore_driver = create_new_data_restore_driver(config, from, delta);
        data_restore_driver
            .load_past_state()
            .expect("Cant get past state");
        data_restore_driver.run_state_updates();
    }
}
