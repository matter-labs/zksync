use std::sync::mpsc::{channel, Sender, Receiver};
use crate::eth_client::{ETHClient, PROD_PLASMA};
use web3::types::{U256, U128, H256};
use crate::models::plasma_models::{TxBlock, Account};
use super::prover::BabyProver;

use crate::primitives::{serialize_fe_for_ethereum};


#[derive(Debug, Clone)]
pub struct Commitment {
    pub new_root: U256,
    pub block_number: U256,
    pub total_fees: U256,
    pub public_data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct EncodedProof {
    pub groth_proof: [U256; 8],
    pub block_number: U256,

    // affected accounts for db update
    pub accounts: Vec<(u32, Account)>,
}

#[derive(Debug, Clone)]
pub enum EthereumTx {
    Commitment(Commitment),
    Proof(EncodedProof),
}

pub fn run_eth_sender() -> Sender<EthereumTx> {
    let (tx_for_eth, rx_for_eth) = channel();
    std::thread::spawn(move || {
        
        let mut eth_client = ETHClient::new(PROD_PLASMA);
        for tx in rx_for_eth {
            match tx {
                EthereumTx::Commitment(commitment) => {
                    println!("Got block commitment");
                    let block_number = commitment.block_number.as_u64();
                    let total_fees = U128::from(commitment.total_fees);
                    let tx_data_packed = commitment.public_data;
                    let new_root: H256 = H256::from(commitment.new_root);
                    println!("Public data = {}", hex::encode(tx_data_packed.clone()));
                    let hash = eth_client.commit_block(block_number, total_fees, tx_data_packed, new_root); 
                    println!("Commitment tx hash = {}", hash.unwrap());
                },
                EthereumTx::Proof(proof) => {
                    println!("Got block proof");
                    let block_number = proof.block_number.as_u64();
                    let proof = proof.groth_proof;
                    let hash = eth_client.verify_block(block_number, proof); 
                    println!("Proving tx hash = {}", hash.unwrap());
                }
            }
        }

    });

    tx_for_eth
}

pub fn run_commitment_pipeline(rx_for_commitments: Receiver<TxBlock>, tx_for_eth: Sender<EthereumTx>) {

    for block in rx_for_commitments {
        
        // TODO: synchronously commit block to storage
        // use block itself

        let new_root = block.new_root_hash.clone();
        println!("Commiting to new root = {}", new_root);
        let block_number = block.block_number;
        let tx_data = BabyProver::encode_transactions(&block).unwrap();
        let tx_data_bytes = tx_data;
        let comittment = Commitment{
            new_root:       serialize_fe_for_ethereum(new_root),
            block_number:   U256::from(block_number),
            total_fees:     U256::from(0),
            public_data:    tx_data_bytes,
        };
        tx_for_eth.send(EthereumTx::Commitment(comittment));
    }
}

pub fn run_proof_pipeline(rx_for_proofs: Receiver<EncodedProof>, tx_for_eth: Sender<EthereumTx>) {

    for proof in rx_for_proofs {
        // TODO: synchronously update accounts in storage
        // use proof.accounts

        tx_for_eth.send(EthereumTx::Proof(proof));
    }
}