#![cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))]

use std::sync::mpsc;
use plasma::models::{TransferTx, PublicKey, Account, Nonce};
use super::models::{StateKeeperRequest, NetworkStatus, TransferTxConfirmation};
use super::storage::{ConnectionPool, StorageProcessor};

use actix_web::{
    middleware, 
    server, 
    App, 
    AsyncResponder, 
    Error, 
    HttpMessage,
    HttpRequest, 
    HttpResponse, 
    middleware::cors::Cors,
    http::Method,
    http::StatusCode,
};

use futures::Future;
use std::env;
use tokio::prelude::*;
use tokio::runtime::Runtime;
use tokio::timer::{Interval, Delay};
use std::time::{Duration, Instant};

#[derive(Debug, Serialize, Deserialize)]
struct TransactionRequest {
    from: u32,
    to: u32,
    amount: u128
}

#[derive(Debug, Serialize, Deserialize)]
struct TransactionResponse {
    accepted:       bool,
    error:          Option<String>,
    confirmation:   Option<TransferTxConfirmation>
}

#[derive(Debug, Serialize, Deserialize)]
struct AccountError {
    error: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TestnetConfigResponse {
    address: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AccountDetailsResponse {
    pending:        Option<Account>,
    verified:       Option<Account>,
    committed:      Option<Account>,
}

// singleton to keep info about channels required for Http server
#[derive(Clone)]
pub struct AppState {
    tx_for_state: mpsc::Sender<StateKeeperRequest>,
    contract_address: String,
    connection_pool: ConnectionPool
}

const TIMEOUT: u64 = 500;

fn handle_submit_tx(req: &HttpRequest<AppState>) -> Box<Future<Item = HttpResponse, Error = Error>> {
    let tx_for_state = req.state().tx_for_state.clone();
    req.json()
        .from_err() // convert all errors into `Error`
        .and_then(move |tx: TransferTx| {
            //println!("New incoming transaction: {:?}", &tx);

            if let Err(error) = tx.validate() {
                println!("Transaction itself is invalid: {}", error);
                let resp = TransactionResponse{
                    accepted:       false,
                    error:          Some(error),
                    confirmation:   None,
                };
                return Ok(HttpResponse::Ok().json(resp));
            }

            // TODO: the code below will block the current thread; switch to futures instead
            let (key_tx, key_rx) = mpsc::channel();
            let request = StateKeeperRequest::GetAccount(tx.from, key_tx);
            tx_for_state.send(request).expect("must send a new transaction to queue");
            // now wait for state_keeper to return a result
            let r = key_rx.recv_timeout(std::time::Duration::from_millis(TIMEOUT));
            if r.is_err() {
                let resp = TransactionResponse{
                    accepted:       false,
                    error:          Some("timeout".to_owned()),
                    confirmation:   None,
                };
                return Ok(HttpResponse::Ok().json(resp));
            }

            let account = r.unwrap();
            let pub_key: Option<PublicKey> = account.and_then( |a| a.get_pub_key() );
            if let None = &pub_key {
                println!("Not public key!");
                let resp = TransactionResponse{
                    accepted:       false,
                    error:          Some("pubkey expired".to_owned()),
                    confirmation:   None,
                };
                return Ok(HttpResponse::Ok().json(resp));
            }

            let pub_key = pub_key.unwrap();
            let verified = tx.verify_sig(&pub_key);
            if !verified {
                let (x, y) = pub_key.0.into_xy();
                println!("Got public key: {:?}, {:?}", x, y);
                println!("Signature is invalid: (x,y,s) = ({:?},{:?},{:?})", &tx.signature.r_x, &tx.signature.r_y, &tx.signature.s);
                let resp = TransactionResponse{
                    accepted:       false,
                    error:          Some("invalid signature".to_owned()),
                    confirmation:   None,
                };
                return Ok(HttpResponse::Ok().json(resp));
            }

            //println!("Signature is valid");
            let mut tx = tx.clone();
            let (add_tx, add_rx) = mpsc::channel();
            tx.cached_pub_key = Some(pub_key);

            tx_for_state.send(StateKeeperRequest::AddTransferTx(tx, add_tx)).expect("must send transaction to sate keeper from rest api");

            // TODO: reconsider timeouts
            let send_result = add_rx.recv_timeout(std::time::Duration::from_millis(500));
            match send_result {
                Ok(result) => match result {
                    Ok(confirmation) => {
                        //println!("Transaction was accepted");
                        let resp = TransactionResponse{
                            accepted:       true,
                            error:          None,
                            confirmation:   Some(confirmation),
                        };
                        Ok(HttpResponse::Ok().json(resp))
                    },
                    Err(error) => {
                        println!("State keeper rejected the transaction");
                        let resp = TransactionResponse{
                            accepted:       false,
                            error:          Some(format!("{:?}", error)),
                            confirmation:   None,
                        };
                        Ok(HttpResponse::Ok().json(resp))
                    },
                },
                Err(_) => {
                    println!("Did not get a result from the state keeper");
                    let resp = TransactionResponse{
                        accepted:       false,
                        error:          Some("result timeout".to_owned()),
                        confirmation:   None,
                    };
                    Ok(HttpResponse::Ok().json(resp))
                }
            }
        })
        .responder()
}

use actix_web::Result as ActixResult;

fn handle_get_account_state(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    let tx_for_state = req.state().tx_for_state.clone();
    let pool = req.state().connection_pool.clone();

    let storage = pool.access_storage();
    if storage.is_err() {
        return Ok(HttpResponse::Ok().json(AccountError{error:"rate limit".to_string()}));
    }
    let storage = storage.unwrap();

    // check that something like this exists in state keeper's memory at all
    let account_id_string = req.match_info().get("id");
    if account_id_string.is_none() {
        return Ok(HttpResponse::Ok().json(AccountError{error:"invalid parameters".to_string()}));
    }
    let account_id = account_id_string.unwrap().parse::<u32>();
    if account_id.is_err(){
        return Ok(HttpResponse::Ok().json(AccountError{error:"invalid account_id".to_string()}));
    }

    let (acc_tx, acc_rx) = mpsc::channel();
    let account_id_u32 = account_id.unwrap();
    let request = StateKeeperRequest::GetAccount(account_id_u32, acc_tx);
    tx_for_state.send(request).expect("must send a request for an account state");
    
    let pending: Result<Option<Account>, _> = acc_rx.recv_timeout(std::time::Duration::from_millis(TIMEOUT));

    if pending.is_err() {
        println!("API request timeout!");
        return Ok(HttpResponse::Ok().json(AccountError{error:"account request timeout".to_string()}));
    }

    let pending = pending.unwrap();
    if pending.is_none() {
        return Ok(HttpResponse::Ok().json(AccountError{error:"non-existing account".to_string()}));
    }

    let committed = storage.last_committed_state_for_account(account_id_u32).expect("last_committed_state_for_account: db must work");
    let verified = storage.last_verified_state_for_account(account_id_u32).expect("last_verified_state_for_account: db must work");

    // QUESTION: why do we need committed here?

    let response = AccountDetailsResponse {
        pending,
        verified,
        committed,
    };

    Ok(HttpResponse::Ok().json(response))
}

fn handle_get_testnet_config(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    let address = req.state().contract_address.clone();
    Ok(HttpResponse::Ok().json(TestnetConfigResponse{
        address: format!("0x{}", address)
    }))
}

fn handle_get_network_status(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    let tx_for_state = req.state().tx_for_state.clone();

    let (tx, rx) = mpsc::channel();
    let request = StateKeeperRequest::GetNetworkStatus(tx);
    tx_for_state.send(request).expect("must send a new transaction to queue");
    let status: Result<NetworkStatus, _> = rx.recv_timeout(std::time::Duration::from_millis(TIMEOUT));
    if status.is_err() {
        return Ok(HttpResponse::Ok().json(AccountError{error: "timeout".to_owned()}));
    }
    let status = status.unwrap();

    let pool = req.state().connection_pool.clone();
    let storage = pool.access_storage();
    if storage.is_err() {
        return Ok(HttpResponse::Ok().json(AccountError{error: "rate limit".to_string()}));
    }
    let mut storage = storage.unwrap();
    
    // TODO: properly handle failures
    let last_committed = storage.get_last_committed_block().unwrap_or(0);
    let last_verified = storage.get_last_verified_block().unwrap_or(0);
    let outstanding_txs = storage.count_outstanding_proofs(last_verified).unwrap_or(0);

    let status = NetworkStatus{
        next_block_at_max: status.next_block_at_max,
        last_committed,
        last_verified,  
        outstanding_txs,
    };

    Ok(HttpResponse::Ok().json(status))
}

fn handle_get_account_transactions(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok().json("{}"))
}

pub fn start_api_server(
    tx_for_state: mpsc::Sender<StateKeeperRequest>,
    connection_pool: ConnectionPool) 
{
    let address = env::var("BIND_TO").unwrap_or("127.0.0.1".to_string());
    let port = env::var("PORT").unwrap_or("8080".to_string());
    let contract_address = env::var("CONTRACT_ADDR").unwrap();
    ::std::env::set_var("RUST_LOG", "actix_web=info");
    std::thread::Builder::new().name("actix".to_string()).spawn(move || {

        let sys = actix::System::new("api-server");
        let server_config = format!("{}:{}", address, port);

        let state = AppState {
            tx_for_state: tx_for_state.clone(),
            contract_address: contract_address.clone(),
            connection_pool: connection_pool.clone(),
        };
        let state2 = std::sync::Arc::new(state.clone());

        //move is necessary to give closure below ownership
        server::new(move || {
            App::with_state(state.clone()) // <- create app with shared state
            .middleware( middleware::Logger::default() )
            .middleware(
                Cors::build()
                    .send_wildcard()
                    .max_age(3600)
                    .finish()
            )
            .scope("/api/v0.1", |api_scope| {
                api_scope
                .resource("/testnet_config", |r| {
                    r.method(Method::GET).f(handle_get_testnet_config);
                })
                .resource("/status", |r| {
                    r.method(Method::GET).f(handle_get_network_status);
                })
                .resource("/submit_tx", |r| {
                    r.method(Method::POST).f(handle_submit_tx);
                })
                .resource("/account/{id}", |r| {
                    r.method(Method::GET).f(handle_get_account_state);
                })
                .resource("/account/{id}/transactions", |r| {
                    r.method(Method::GET).f(handle_get_account_transactions);
                })
            })
        }).bind(&server_config)
        .unwrap()
        .shutdown_timeout(1)
        .start();

        let state_checker = Interval::new(Instant::now(), Duration::from_millis(1000))
            .fold(state2, |mut state, instant| {
                let state = state.clone();
                let pool = state.connection_pool.clone();

                let storage = pool.access_storage();
                if storage.is_err() {
                    panic!("oops");
                }
                let mut storage = storage.unwrap();
                
                // TODO: properly handle failures
                let last_committed = storage.get_last_committed_block().unwrap_or(0);
                let last_verified = storage.get_last_verified_block().unwrap_or(0);
                let outstanding_txs = storage.count_outstanding_proofs(last_verified).unwrap_or(0);

                let status = NetworkStatus{
                    next_block_at_max: None,
                    last_committed,
                    last_verified,  
                    outstanding_txs,
                };

                println!("status from db: {:?}", status);
                Delay::new(Instant::now() + Duration::from_millis(5000)).and_then(move |_| Ok(state))
            })
            .map(|_| ())
            .map_err(|e| panic!("interval errored; err={:?}", e));

        println!("Started http server: {}", server_config);
        actix::System::with_current( |_| {
            actix::spawn(state_checker);
        });

        sys.run();
    });
}