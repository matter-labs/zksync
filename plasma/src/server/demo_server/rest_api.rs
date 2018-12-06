#![cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))]

extern crate actix;
extern crate actix_web;
extern crate futures;
extern crate serde_json;
extern crate serde_derive;

use std::thread;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use std::time;

use super::super::prover::Prover;

use web3::types::{U256, Bytes};

use std::collections::HashMap;
use ff::{Field, PrimeField};
use sapling_crypto::eddsa::{PrivateKey, PublicKey};

use super::state_keeper::{TxInfo, PlasmaStateKeeper};

use pairing::bn256::Bn256;
use super::super::plasma_state::{Block};
use super::super::super::balance_tree::BabyBalanceTree;

use super::super::super::primitives::{serialize_g1_for_ethereum, serialize_g2_for_ethereum, serialize_fe_for_ethereum, field_element_to_u32};


use self::actix_web::{
    error, http, middleware, server, App, AsyncResponder, Error, HttpMessage,
    HttpRequest, HttpResponse, Json,
};

use futures::{Future, Stream};

use super::super::baby_prover::{BabyProver, EthereumProof};

#[derive(Debug, Serialize, Deserialize)]
struct TransactionRequest {
    from: u32,
    to: u32,
    amount: u128
}

#[derive(Debug, Serialize, Deserialize)]
struct TransactionResponse {
    accepted: bool,
}

// static STATE_KEEPER_TX: &AppState = &AppState{state_keeper_tx: None};

// singleton to keep info about channels required for Http server
#[derive(Clone)]
struct AppState {
    state_keeper_tx: mpsc::Sender<(TxInfo, mpsc::Sender<bool>)>,
}

fn send_transaction(req: &HttpRequest<AppState>) -> Box<Future<Item = HttpResponse, Error = Error>> {
    let state_tx = req.state().state_keeper_tx.clone();
    req.json()
        .from_err()  // convert all errors into `Error`
        .and_then(move |val: TransactionRequest| {
            let (tx, rx) = mpsc::channel::<bool>();
            let info = TxInfo {
                from: val.from,
                to: val.to,
                amount: val.amount,
                fee: 0,
                nonce: 0,
                good_until_block: 0
            };
            state_tx.send((info, tx.clone()));
            let result = rx.recv();
            let resp = TransactionResponse{
                accepted: result.unwrap()
            };
            Ok(HttpResponse::Ok().json(resp))  // <- send response
        })
        .responder()
}

#[test]
fn test_run_server() {
    // create channel to accept deserialized requests for new transacitons

    let (tx_for_transactions, rx_for_transactions) = mpsc::channel::<(TxInfo, mpsc::Sender<bool>)>();
    let (tx_for_blocks, rx_for_blocks) = mpsc::channel::<Block<Bn256>>();
    let (tx_for_proofs, rx_for_proofs) = mpsc::channel::<EthereumProof>();
    let (tx_for_tx_data, rx_for_tx_data) = mpsc::channel::<EthereumProof>();

    ::std::env::set_var("RUST_LOG", "actix_web=info");
    let sys = actix::System::new("ws-example");

    // here we should insert default accounts into the tree
    let tree = BabyBalanceTree::new(24);
    let root = tree.root_hash();
    let keys_map = HashMap::<u32,PrivateKey<Bn256>>::new();
    let mut keeper = PlasmaStateKeeper {
        balance_tree: tree,
        block_number: 0,
        root_hash:    root,
        transactions_channel: rx_for_transactions,
        batch_channel: tx_for_blocks.clone(),
        batch_size : 8,
        current_batch: vec![],
        private_keys: keys_map
    };

    let mut prover = BabyProver::create(&keeper).unwrap();

    // spawn a thread with a state processor

    let state_handle = thread::spawn(move || {
        keeper.run();
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
                let block_number = block.block_number;
                let tx_data = BabyProver::encode_transactions(&block).unwrap();
                let tx_data_bytes = Bytes::from(tx_data);
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

    // TODO: take rs_for_tx_data and re_for_proofs and use it in separate loop for ethereum commitments

    //move is necessary to give closure below ownership
    server::new(move || {
        App::with_state(AppState {
        state_keeper_tx: tx_for_transactions.clone()
    }.clone()) // <- create app with shared state
            // enable logger
            .middleware(middleware::Logger::default())
            // register simple handler, handle all methods
            .resource("/send", |r| r.method(http::Method::POST).f(send_transaction))
    }).bind("127.0.0.1:8080")
    .unwrap()
    .shutdown_timeout(1)
    .start();

    println!("Started http server: 127.0.0.1:8080");
    let _ = sys.run();
    // state_handle.join();
}