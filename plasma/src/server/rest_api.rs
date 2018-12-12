#![cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))]

use std::thread;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use std::time;
use rand::{SeedableRng, Rng, XorShiftRng};
use web3::types::{U256, Bytes, U128, H256};
use std::collections::HashMap;
use ff::{Field, PrimeField};
use sapling_crypto::eddsa::{PrivateKey, PublicKey};
use sapling_crypto::jubjub::{FixedGenerators};
use sapling_crypto::alt_babyjubjub::{AltJubjubBn256};
use super::state_keeper::{TxInfo, PlasmaStateKeeper};
use pairing::bn256::{Bn256, Fr};
use crate::models::state::{State};
use super::baby_models::{Account, AccountTree, Block};
use crate::primitives::{serialize_g1_for_ethereum, serialize_g2_for_ethereum, serialize_fe_for_ethereum, field_element_to_u32};
use crate::eth_client::{ETHClient, PROD_PLASMA};
use super::prover::{BabyProver, EthereumProof};
use crate::models::params;

use actix_web::{
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
pub struct AppState {
    state_keeper_tx: mpsc::Sender<(TxInfo, mpsc::Sender<bool>)>,
}

pub fn handle_send_transaction(req: &HttpRequest<AppState>) -> Box<Future<Item = HttpResponse, Error = Error>> {
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

pub fn start_api_server(tx_for_transactions: mpsc::Sender<(TxInfo, mpsc::Sender<bool>)>) {
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
                        r.method(Method::POST).f(handle_send_transaction);
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
}