use std::sync::mpsc::{channel, Sender, Receiver};
use crate::eth_client::{ETHClient, TxMeta, PROD_PLASMA};
use web3::types::{U256, U128, H256};
use crate::models::{Block, TransferBlock, Account, BatchNumber, AccountMap};
use super::prover::BabyProver;
use super::storage::StorageConnection;
use serde_json::{to_value, value::Value};
use crate::primitives::{serialize_fe_for_ethereum};

//#[derive(Debug, Clone, Serialize, Deserialize)]
pub type EncodedProof = [U256; 8];

pub struct BlockProof(pub EncodedProof, pub Vec<(u32, Account)>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EthBlockData {
    Transfer{
        total_fees:     U128,
        // TODO: with serde bytes
        public_data:    Vec<u8>,
    },
    Deposit{
        batch_number:   BatchNumber,
    },
    Exit{
        batch_number:   BatchNumber,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Operation {
    Commit{
        block_number:       u32, 
        new_root:           H256, 
        block_data:         EthBlockData,
        accounts_updated:   AccountMap,
    },
    Verify{
        block_number:       u32, 
        proof:              EncodedProof, 
        block_data:         EthBlockData,
        accounts_updated:   AccountMap,
    },
    StartDepositBatch,
    StartExitBatch,
    // ...
}

fn keys_sorted(accounts_updated: AccountMap) -> Vec<u64> {
    let mut acc: Vec<u64> = accounts_updated.keys()
        .map(|&k| k as u64)
        .collect();
    acc.sort();
    acc
} 

pub fn start_eth_sender() -> Sender<(Operation, TxMeta)> {
    let (tx_for_eth, rx_for_eth) = channel();
    std::thread::spawn(move || {
        let mut eth_client = ETHClient::new(PROD_PLASMA);
        for (op, meta) in rx_for_eth {
            println!("Operation requested: {:?}", &op);
            let tx = match op {
                Operation::Commit{block_number, new_root, block_data, accounts_updated} => {
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
                Operation::Verify{block_number, proof, block_data, accounts_updated} => {
                    match block_data {
                        EthBlockData::Transfer{total_fees, public_data} =>
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

pub fn run_committer(rx_for_ops: Receiver<Operation>, tx_for_eth: Sender<(Operation, TxMeta)>) {

    let storage = StorageConnection::new();
    for op in rx_for_ops {
        // persist in storage first
        
        // TODO: with postgres transaction
        let (addr, nonce) = storage.commit_op(&op).unwrap();
        match &op {
            Operation::Commit{block_number, new_root, block_data, accounts_updated} => 
                storage.commit_state_update(*block_number, accounts_updated),
            Operation::Verify{block_number, proof, block_data, accounts_updated} => 
                storage.apply_state_update(*block_number),
            _ => unimplemented!(),
        };

        // submit to eth
        tx_for_eth.send((op, TxMeta{addr, nonce}));
    }
}


// pub fn run_commitment_pipeline(rx_for_commitments: Receiver<TransferBlock>, tx_for_eth: Sender<EthereumTx>) {

//     let storage = StorageConnection::new();
//     for block in rx_for_commitments {
//         // synchronously commit block to storage
//         let r = storage.store_block(block.block_number as i32, &to_value(&block).unwrap()).expect("database failed");

//         let new_root = block.new_root_hash.clone();
//         println!("Commiting to new root = {}", new_root);
//         let block_number = block.block_number;
//         let tx_data = BabyProver::encode_transfer_transactions(&block).unwrap();
//         let tx_data_bytes = tx_data;
//         let comittment = Commitment{
//             new_root:       serialize_fe_for_ethereum(new_root),
//             block_number:   U256::from(block_number),
//             total_fees:     U256::from(0),
//             public_data:    tx_data_bytes,
//         };
//         tx_for_eth.send(EthereumTx::Commitment(comittment));
//     }
// }

// pub fn run_proof_pipeline(rx_for_proofs: Receiver<BlockProof>, tx_for_eth: Sender<EthereumTx>) {

//     let storage = StorageConnection::new();
//     for msg in rx_for_proofs {

//         let BlockProof(proof, accounts) = msg;

//         // synchronously commit proof and update accounts in storage
//         let block_number: i32 = proof.block_number.as_u32() as i32;
//         storage.store_proof(block_number, &to_value(&proof).unwrap());
//         storage.update_accounts(accounts);

//         tx_for_eth.send(EthereumTx::Proof(proof));
//     }
// }