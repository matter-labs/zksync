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
use rand::{SeedableRng, Rng, XorShiftRng};

use web3::types::{U256, Bytes, U128, H256};

use std::collections::HashMap;
use ff::{Field, PrimeField};
use sapling_crypto::eddsa::{PrivateKey, PublicKey};
use sapling_crypto::jubjub::{FixedGenerators};
use sapling_crypto::alt_babyjubjub::{AltJubjubBn256};

use super::state_keeper::{TxInfo, PlasmaStateKeeper};

use pairing::bn256::{Bn256, Fr};
use super::super::plasma_state::{Block, State};
use super::super::super::balance_tree::{BabyBalanceTree, BabyLeaf};
use super::super::super::circuit::{plasma_constants};

use super::super::super::primitives::{serialize_g1_for_ethereum, serialize_g2_for_ethereum, serialize_fe_for_ethereum, field_element_to_u32};

use super::super::eth::{ETHClient, PROD_PLASMA};

use self::actix_web::{
    error, 
    middleware, 
    server, 
    App, 
    AsyncResponder, 
    Error, 
    HttpMessage,
    HttpRequest, 
    HttpResponse, 
    Json,     
    middleware::cors::Cors,
    http::{header, Method},
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
                good_until_block: 100
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
    run();
}

pub fn run() {
    // create channel to accept deserialized requests for new transacitons

    let (tx_for_transactions, rx_for_transactions) = mpsc::channel::<(TxInfo, mpsc::Sender<bool>)>();
    let (tx_for_blocks, rx_for_blocks) = mpsc::channel::<Block<Bn256>>();
    let (tx_for_proofs, rx_for_proofs) = mpsc::channel::<EthereumProof>();
    let (tx_for_tx_data, rx_for_tx_data) = mpsc::channel::<EthereumProof>();

    ::std::env::set_var("RUST_LOG", "actix_web=info");
    let sys = actix::System::new("ws-example");

    println!("Creating ETH client");

    let mut eth_client = ETHClient::new(PROD_PLASMA);
    eth_client.get_first_nonce();

    println!("Creating default state");

    // here we should insert default accounts into the tree
    let tree_depth = *plasma_constants::BALANCE_TREE_DEPTH as u32;
    let mut tree = BabyBalanceTree::new(tree_depth);

    let number_of_accounts = 1000;

    let mut keys_map = HashMap::<u32,PrivateKey<Bn256>>::new();
    {
        let mut_tree = & mut tree;
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let params = &AltJubjubBn256::new();
        let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

        let default_balance_string = "1000000";

        for i in 0..number_of_accounts {
            let leaf_number : u32 = i;

            let sk = PrivateKey::<Bn256>(rng.gen());
            let pk = PublicKey::from_private(&sk, p_g, params);
            let (x, y) = pk.0.into_xy();

            keys_map.insert(i, sk);

            let leaf = BabyLeaf {
                balance:    Fr::from_str(default_balance_string).unwrap(),
                nonce:      Fr::zero(),
                pub_x:      x,
                pub_y:      y,
            };

            mut_tree.insert(leaf_number, leaf.clone());
        }

    }

    let mut keeper = PlasmaStateKeeper {
        balance_tree: tree,
        block_number: 1,
        // root_hash:    root,
        transactions_channel: rx_for_transactions,
        batch_channel: tx_for_blocks.clone(),
        batch_size : 32,
        current_batch: vec![],
        private_keys: keys_map
    };

    let root = keeper.root_hash();

    println!("Created state keeper with  {} accounts with balances, root hash = {}", number_of_accounts, root);


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

    //move is necessary to give closure below ownership
    server::new(move || {
        App::with_state(AppState {
        state_keeper_tx: tx_for_transactions.clone()
    }.clone()) // <- create app with shared state
            // enable logger
            .middleware(middleware::Logger::default())
            // enable CORS
            .configure(|app| {
                Cors::for_app(app)
                    // .allowed_origin("*")
                    .send_wildcard()
                    // .allowed_methods(vec!["GET", "POST", "OPTIONS"])
                    // .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
                    // .allowed_header(header::CONTENT_TYPE)
                    .max_age(3600)
                    .resource("/send", |r| {
                        r.method(Method::POST).f(send_transaction);
                        r.method(Method::OPTIONS).f(|_| HttpResponse::Ok());
                        r.method(Method::GET).f(|_| HttpResponse::Ok());
                    })
                    .register()
            })
    }).bind("127.0.0.1:8080")
    .unwrap()
    .shutdown_timeout(1)
    .start();

    println!("Started http server: 127.0.0.1:8080");
    let _ = sys.run();
    // state_handle.join();
}