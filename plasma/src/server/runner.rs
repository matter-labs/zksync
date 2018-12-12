use std::thread;
use std::sync::mpsc::{channel, Sender};

use super::prover::BabyProver;
use super::state_keeper::PlasmaStateKeeper;
use super::rest_api::run_api_server;
use super::committer::{run_committer, EthereumProof};

use crate::models::plasma_models::Block;

use crate::models::tx::TxUnpacked;
use crate::primitives::serialize_fe_for_ethereum;

pub fn run() {

    // create channel to accept deserialized requests for new transacitons

    let (tx_for_transactions, rx_for_transactions) = channel::<(TxUnpacked, Sender<bool>)>();
    let (tx_for_blocks, rx_for_blocks) = channel::<Block>();
    let (tx_for_proofs, rx_for_proofs) = channel::<EthereumProof>();

    let mut keeper = PlasmaStateKeeper::new();
    let mut prover = BabyProver::create(&keeper.state).unwrap();

    // spawn threads for different processes

    // runs the server which will handling incoming REST API requests
    // tx requests are forwarded to state keeper
    thread::spawn(move || {  
        run_api_server(tx_for_transactions);
    });

    // applies incoming transactions to the state and adds them to the batch basket
    // once the basket is full, the new block is passed to the prover to generate proof
    thread::spawn(move || {
        keeper.run(rx_for_transactions, tx_for_blocks);
    });

    // takes the block to prove, encodes its data for ethereum and passes it to the committer
    // then the proof is generated and also passed to committer
    thread::spawn(move || {
        prover.run(rx_for_blocks, tx_for_proofs);
    });

    // hanldes eth operations: submits eth transactions to commit and verify blocks
    run_committer(rx_for_proofs);
}
