use std::thread;
use std::sync::mpsc::{channel, Sender};

use super::prover::{BabyProver};
use super::state_keeper::{PlasmaStateKeeper, BlockProcessingRequest};
use super::rest_api::run_api_server;
use super::committer::{self, Commitment, EncodedProof};
use super::mem_pool::MemPool;
use super::eth_watch::EthWatch;

use crate::models::plasma_models::{Block, TxBlock};

use crate::models::tx::TxUnpacked;
use crate::primitives::serialize_fe_for_ethereum;

pub fn run() {

    // create channel to accept deserialized requests for new transacitons

    let (tx_for_tx, rx_for_tx) = channel::<TxUnpacked>();
    let (tx_for_blocks, rx_for_blocks) = channel::<BlockProcessingRequest>();
    let (tx_for_proof_requests, rx_for_proof_requests) = channel::<Block>();
    let (tx_for_commitments, rx_for_commitments) = channel::<TxBlock>();
    let (tx_for_proofs, rx_for_proofs) = channel::<BlockProof>();

    let mut mem_pool = MemPool::new();
    let mut state_keeper = PlasmaStateKeeper::new();
    let mut prover = BabyProver::create(&state_keeper.state).unwrap();
    let mut eth_watch = EthWatch::new();

    // spawn threads for different processes
    // see https://docs.google.com/drawings/d/16UeYq7cuZnpkyMWGrgDAbmlaGviN2baY1w1y745Me70/edit?usp=sharing

    thread::spawn(move || {
        run_api_server(tx_for_tx);
    });

    let tx_for_blocks2 = tx_for_blocks.clone();
    thread::spawn(move || {  
        mem_pool.run(rx_for_tx, tx_for_blocks2);
    });

    thread::spawn(move || {  
        eth_watch.run(tx_for_blocks);
    });

    thread::spawn(move || {
        state_keeper.run(rx_for_blocks, tx_for_commitments, tx_for_proof_requests);
    });

    thread::spawn(move || {
        prover.run(rx_for_proof_requests, tx_for_proofs);
    });

    let tx_for_eth = committer::run_eth_sender();
    let tx_for_eth2 = tx_for_eth.clone();

    thread::spawn(move || {
        committer::run_commitment_pipeline(rx_for_commitments, tx_for_eth.clone());
    });

    // no thread::spawn for the last processor
    committer::run_proof_pipeline(rx_for_proofs, tx_for_eth2);
}
