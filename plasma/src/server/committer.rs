use std::sync::mpsc::Receiver;
use crate::eth_client::{ETHClient, PROD_PLASMA};
use web3::types::{U256, U128, H256};

#[derive(Debug, Clone)]
pub struct Commitment {
    pub new_root: U256,
    pub block_number: U256,
    pub total_fees: U256,
    pub public_data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Proof {
    pub groth_proof: [U256; 8],
    pub block_number: U256,
}

#[derive(Debug, Clone)]
pub enum EthereumProof {
    Commitment(Commitment),
    Proof(Proof),
}

pub fn run_committer(rx_for_proofs: Receiver<EthereumProof>) {

    let mut eth_client = ETHClient::new(PROD_PLASMA);

    for proof in rx_for_proofs {
        match proof {
            EthereumProof::Commitment(commitment) => {
                println!("Got block commitment");
                let block_number = commitment.block_number.as_u64();
                let total_fees = U128::from(commitment.total_fees);
                let tx_data_packed = commitment.public_data;
                let new_root: H256 = H256::from(commitment.new_root);
                println!("Public data = {}", hex::encode(tx_data_packed.clone()));
                let hash = eth_client.commit_block(block_number, total_fees, tx_data_packed, new_root); 
                println!("Commitment tx hash = {}", hash.unwrap());
            },
            EthereumProof::Proof(proof) => {
                println!("Got block proof");
                let block_number = proof.block_number.as_u64();
                let proof = proof.groth_proof;
                let hash = eth_client.verify_block(block_number, proof); 
                println!("Proving tx hash = {}", hash.unwrap());
            }
        }
    }
}