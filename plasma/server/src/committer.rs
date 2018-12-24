use std::sync::mpsc::{channel, Sender, Receiver};
use plasma::eth_client::{ETHClient, TxMeta, PROD_PLASMA};
use plasma::models::AccountMap;
use super::storage::StorageConnection;
use super::models::*;

fn keys_sorted(accounts_updated: AccountMap) -> Vec<u64> {
    let mut acc: Vec<u64> = accounts_updated.keys()
        .map(|&k| k as u64)
        .collect();
    acc.sort();
    acc
} 

pub fn start_eth_sender() -> Sender<(EthOperation, TxMeta)> {
    let (tx_for_eth, rx_for_eth) = channel();
    let mut eth_client = ETHClient::new(PROD_PLASMA);

    load_pendings_ops(&eth_client, &tx_for_eth);

    std::thread::spawn(move || {
        for (op, meta) in rx_for_eth {
            println!("EthOperation requested: {:?}", &op);
            let tx = match op {
                EthOperation::Commit{block_number, new_root, block_data, accounts_updated} => {
                    match block_data {
                        EthBlockData::Transfer{total_fees, public_data} =>
                            eth_client.call("commitTransferBlock", meta, 
                                (block_number as u64, total_fees, public_data, new_root)),
                        EthBlockData::Deposit{batch_number} =>
                            eth_client.call("commitDepositBlock", meta,
                                (block_number as u64, batch_number as u64, keys_sorted(accounts_updated))),
                        EthBlockData::Exit{batch_number} =>
                            eth_client.call("commitExitBlock", meta,
                                (block_number as u64, batch_number as u64, keys_sorted(accounts_updated))),
                    }
                },
                EthOperation::Verify{block_number, proof, block_data, accounts_updated} => {
                    match block_data {
                        EthBlockData::Transfer{total_fees: _, public_data: _} =>
                            eth_client.call("verifyTransferBlock", meta,
                                (block_number as u64, proof)),
                        EthBlockData::Deposit{batch_number} =>
                            eth_client.call("verifyDepositBlock", meta,
                                (block_number as u64, batch_number as u64, keys_sorted(accounts_updated))),
                        EthBlockData::Exit{batch_number} =>
                            eth_client.call("verifyExitBlock", meta,
                                (block_number as u64, batch_number as u64, keys_sorted(accounts_updated))),
                    }
                },
                _ => unimplemented!(),
            };
            // TODO: process tx sending failure
            println!("Commitment tx hash = {}", tx.unwrap());
        }
    });
    tx_for_eth
}

pub fn load_pendings_ops(eth_client: &ETHClient, tx_for_eth: &Sender<(EthOperation, TxMeta)>) {
    
    let storage = StorageConnection::new();

    // execute pending transactions
    let current_nonce = eth_client.get_nonce(&eth_client.default_account()).unwrap();
    let ops = storage.load_pendings_ops(current_nonce.as_u32());
    for pending_op in ops {
        let op = serde_json::from_value(pending_op.data).unwrap();
        tx_for_eth.send((op, TxMeta{
            addr:   pending_op.addr, 
            nonce:  pending_op.nonce as u32,
        })).expect("queue must work");
    }
}

pub fn run_committer(rx_for_ops: Receiver<EthOperation>, tx_for_eth: Sender<(EthOperation, TxMeta)>) {

    let storage = StorageConnection::new();
    for op in rx_for_ops {
        // persist in storage first
        
        // TODO: with postgres transaction
        let committed_op = storage.commit_op(&op).expect("db must be functional");

        // submit to eth
        tx_for_eth.send((op, TxMeta{
            addr:   committed_op.addr, 
            nonce:  committed_op.nonce as u32,
        })).expect("queue must work");
    }
}
