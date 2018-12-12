use std::thread;
use std::sync::mpsc;
use std::time;
use std::collections::HashMap;

use web3::types::{U256, U128, H256};
use ff::{Field, PrimeField};
use sapling_crypto::jubjub::{FixedGenerators};

use pairing::bn256::{Bn256, Fr};

use crate::models::plasma_models::{Account, AccountTree, Block, PlasmaState};
use super::prover::{BabyProver, EthereumProof};
use super::state_keeper::{PlasmaStateKeeper};
use super::rest_api::start_api_server;

use crate::models::tx::TxUnpacked;
use crate::primitives::serialize_fe_for_ethereum;
use crate::eth_client::{ETHClient, PROD_PLASMA};


pub fn run() {
    // create channel to accept deserialized requests for new transacitons

    let (tx_for_transactions, rx_for_transactions) = mpsc::channel::<(TxUnpacked, mpsc::Sender<bool>)>();
    let (tx_for_blocks, rx_for_blocks) = mpsc::channel::<Block>();
    let (tx_for_proofs, rx_for_proofs) = mpsc::channel::<EthereumProof>();
    let (tx_for_tx_data, rx_for_tx_data) = mpsc::channel::<EthereumProof>();

    let mut eth_client = ETHClient::new(PROD_PLASMA);
    eth_client.get_first_nonce();

    let mut keeper = PlasmaStateKeeper::new();
    let mut prover = BabyProver::create(&keeper.state).unwrap();

    // spawn a thread with a state processor

    let state_handle = thread::spawn(move || {
        keeper.run(rx_for_transactions, tx_for_blocks);
    });

    let prover_handle = thread::spawn(move || {
        loop {
            let message = rx_for_blocks.try_recv();
            if message.is_err() {
                thread::sleep(time::Duration::from_millis(10));
                continue;
            }
            let block = message.unwrap();
            println!("Got batch!");
            {
                let new_root = block.new_root_hash.clone();
                println!("Commiting to new root = {}", new_root);
                let block_number = block.block_number;
                let tx_data = BabyProver::encode_transactions(&block).unwrap();
                let tx_data_bytes = tx_data;
                let incomplete_proof = EthereumProof {
                    groth_proof: [U256::from(0); 8],
                    new_root: serialize_fe_for_ethereum(new_root),
                    block_number: U256::from(block_number),
                    total_fees: U256::from(0),
                    public_data: tx_data_bytes,
                };
                tx_for_tx_data.send(incomplete_proof);
            }
            let proof = prover.apply_and_prove(&block).unwrap();
            let full_proof = BabyProver::encode_proof(&proof).unwrap();
            tx_for_proofs.send(full_proof);
        }
    });

    let committer_handle = thread::spawn(move || {
        loop {
            {
                let message = rx_for_tx_data.try_recv();
                if message.is_ok() {
                    println!("Got transaction data");
                    let commitment = message.unwrap();
                    let block_number = commitment.block_number.as_u64();
                    let total_fees = U128::from(commitment.total_fees);
                    let tx_data_packed = commitment.public_data;
                    let new_root: H256 = H256::from(commitment.new_root);
                    println!("Will try to commit");
                    println!("Public data = {}", hex::encode(tx_data_packed.clone()));
                    let hash = eth_client.commit_block(block_number, total_fees, tx_data_packed, new_root); 
                    println!("Commitment tx hash = {}", hash.unwrap());
                    continue;
                }
            }
            {
                let message = rx_for_proofs.try_recv();
                if message.is_ok() {
                    println!("Got proof");
                    let proof = message.unwrap();
                    let block_number = proof.block_number.as_u64();
                    let proof = proof.groth_proof;

                    println!("Will try to prove commit");
                    // for i in 0..8 {
                    //     println!("Proof element {} = {}", i, proof[i]);
                    // }
                    let hash = eth_client.verify_block(block_number, proof); 
                    println!("Proving tx hash = {}", hash.unwrap());
                    continue;
                }
            }
            thread::sleep(time::Duration::from_millis(10));
        }
    });

    start_api_server(tx_for_transactions);
}