use std::thread;
use std::sync::mpsc::{channel, Sender};

use super::prover::{BabyProver};
use super::state_keeper::{PlasmaStateKeeper, StateProcessingRequest};
use super::rest_api::run_api_server;
use super::committer::{self, Operation};
use super::mem_pool::MemPool;
use super::eth_watch::EthWatch;

use crate::models::{Block, TransferBlock, TransferTx};

use crate::primitives::serialize_fe_for_ethereum;

pub fn run() {

    // create channel to accept deserialized requests for new transacitons

    let (tx_for_tx, rx_for_tx) = channel::<TransferTx>();
    let (tx_for_state, rx_for_state) = channel::<StateProcessingRequest>();
    let (tx_for_proof_requests, rx_for_proof_requests) = channel::<Block>();
    let (tx_for_ops, rx_for_ops) = channel::<Operation>();

    let mut mem_pool = MemPool::new();
    let mut state_keeper = PlasmaStateKeeper::new();
    let mut prover = BabyProver::create(&state_keeper.state).unwrap();
    let mut eth_watch = EthWatch::new(0, 0);

    // spawn threads for different processes
    // see https://docs.google.com/drawings/d/16UeYq7cuZnpkyMWGrgDAbmlaGviN2baY1w1y745Me70/edit?usp=sharing

    println!("starting actors");

    start_api_server(tx_for_tx, tx_for_state.clone());
    mem_pool.start(rx_for_tx, tx_for_state.clone());
    eth_watch.start(tx_for_state);
    
    state_keeper.start(rx_for_state, tx_for_ops.clone(), tx_for_proof_requests);
    prover.start(rx_for_proof_requests, tx_for_ops);

    let tx_for_eth = committer::start_eth_sender();
    committer::run_committer(rx_for_commitments, tx_for_eth);
}