use std::sync::mpsc::{channel, Sender, Receiver};
use plasma::eth_client::{ETHClient, TxMeta, TEST_PLASMA_ALWAYS_VERIFY};
use plasma::models::{Block, AccountMap, params};
use super::storage::{ConnectionPool, StorageProcessor};
use super::server_models::*;
use web3::types::{U256, H256};
use super::config;

fn sorted_and_padded_for_deposits(accounts_updated: AccountMap) -> [u64; config::DEPOSIT_BATCH_SIZE] {
    let mut tmp = [params::SPECIAL_ACCOUNT_DEPOSIT as u64; config::DEPOSIT_BATCH_SIZE];
    let mut acc: Vec<u64> = accounts_updated.keys()
        .map(|&k| k as u64)
        .collect();
    acc.sort();

    for (i, a) in acc.into_iter().enumerate() {
        tmp[i] = a;
    }

    tmp
}

fn sorted_and_padded_for_exits(accounts_updated: AccountMap) -> [u64; config::EXIT_BATCH_SIZE] {
    let mut tmp = [params::SPECIAL_ACCOUNT_EXIT as u64; config::EXIT_BATCH_SIZE];
    let mut acc: Vec<u64> = accounts_updated.keys()
        .map(|&k| k as u64)
        .collect();
    acc.sort();

    for (i, a) in acc.into_iter().enumerate() {
        tmp[i] = a;
    }

    tmp
}

fn keys_sorted(accounts_updated: AccountMap) -> Vec<u64> {
    let mut acc: Vec<u64> = accounts_updated.keys()
        .map(|&k| k as u64)
        .collect();
    acc.sort();
    acc
} 


fn run_eth_sender(rx_for_eth: Receiver<(Operation, TxMeta)>, mut eth_client: ETHClient) {
    for (op, meta) in rx_for_eth {
        println!("Operation requested"); // println!("Operation requested: {:?}", &op);
        let tx = match op.action {
            Action::Commit{new_root, block: _} => {
                match op.block_data {
                    EthBlockData::Transfer{total_fees, public_data} =>
                        eth_client.call("commitTransferBlock", meta, 
                            (op.block_number as u64, total_fees, public_data, new_root)),

                    EthBlockData::Deposit{batch_number} =>
                        eth_client.call("commitDepositBlock", meta,
                            (U256::from(batch_number), sorted_and_padded_for_deposits(op.accounts_updated), op.block_number as u64, new_root)),

                    EthBlockData::Exit{batch_number, public_data} =>
                        eth_client.call("commitExitBlock", meta,
                            (U256::from(batch_number), sorted_and_padded_for_exits(op.accounts_updated), op.block_number as u64, public_data, new_root)),
                }
            },
            Action::Verify{proof} => {
                match op.block_data {
                    EthBlockData::Transfer{total_fees: _, public_data: _} =>
                        eth_client.call("verifyTransferBlock", meta,
                            (op.block_number as u64, proof)),

                    EthBlockData::Deposit{batch_number} =>
                        eth_client.call("verifyDepositBlock", meta,
                            (U256::from(batch_number), sorted_and_padded_for_deposits(op.accounts_updated), op.block_number as u64, proof)),

                    EthBlockData::Exit{batch_number, public_data: _} =>
                        eth_client.call("verifyExitBlock", meta,
                            (batch_number as u64, op.block_number as u64, proof)),
                }
            },
            _ => unimplemented!(),
        };
        // TODO: process tx sending failure
        if tx.is_err() {
            println!("Error sending tx {}", tx.err().unwrap());

        } else {
            println!("Commitment tx hash = {}", tx.unwrap());
        }

    }
}

pub fn start_eth_sender(pool: ConnectionPool) -> Sender<(Operation, TxMeta)> {
    let (tx_for_eth, rx_for_eth) = channel::<(Operation, TxMeta)>();
    let mut eth_client = ETHClient::new(TEST_PLASMA_ALWAYS_VERIFY);
    let current_nonce = eth_client.get_nonce(&eth_client.default_account()).unwrap();

    let connection = pool.pool.get().expect("committer must connect to db");
    let storage = StorageProcessor::from_connection(connection);

    // TODO: this is for test only, introduce a production switch (as we can not rely on debug/release mode because performance is required for circuits)
    let addr = std::env::var("SENDER_ACCOUNT").unwrap_or("e5d0efb4756bd5cdd4b5140d3d2e08ca7e6cf644".to_string());
    storage.reset_op_config(&addr, current_nonce.as_u32());

    // execute pending transactions
    let ops = storage.load_pendings_txs(current_nonce.as_u32()).expect("db must be functional");
    for pending_op in ops {
        let op = serde_json::from_value(pending_op.data).unwrap();
        tx_for_eth.send((op, TxMeta{
            addr:   pending_op.addr, 
            nonce:  pending_op.nonce as u32,
        })).expect("must send a request for ethereum transaction for pending operations");
    }

    std::thread::Builder::new().name("eth_sender".to_string()).spawn(move || {
        run_eth_sender(rx_for_eth, eth_client);
    });

    tx_for_eth
}
