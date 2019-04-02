#![cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))]

use std::sync::mpsc;
use plasma::models::{TransferTx, PublicKey, Account, Nonce};
use super::server_models::{StateKeeperRequest, NetworkStatus, TransferTxConfirmation};
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
use std::time::Instant;

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

fn handle_submit_tx(req: &HttpRequest<AppState>) -> Box<Future<Item = HttpResponse, Error = Error>> {
    let tx_for_state = req.state().tx_for_state.clone();
    req.json()
        .from_err() // convert all errors into `Error`
        .and_then(move |tx: TransferTx| {
            println!("New incoming transaction: {:?}", &tx);

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
            let account = key_rx.recv_timeout(std::time::Duration::from_millis(100)).expect("must get public key back");

            let pub_key: Option<PublicKey> = account.and_then( |a| a.get_pub_key() );
            if let Some(pk) = pub_key.clone() {
                let (x, y) = pk.0.into_xy();
                println!("Got public key: {:?}, {:?}", x, y);
            }

            let verified = pub_key.as_ref().map( |pk| tx.verify_sig(pk) ).unwrap_or(false);
            if !verified {
                println!("Signature is invalid");
                let resp = TransactionResponse{
                    accepted:       false,
                    error:          Some("invalid signature".to_owned()),
                    confirmation:   None,
                };
                return Ok(HttpResponse::Ok().json(resp));
            }

            println!("Signature is valid");
            let mut tx = tx.clone();
            let (add_tx, add_rx) = mpsc::channel();
            tx.cached_pub_key = pub_key;

            tx_for_state.send(StateKeeperRequest::AddTransferTx(tx, add_tx)).expect("must send transaction to sate keeper from rest api");

            // TODO: reconsider timeouts
            let send_result = add_rx.recv_timeout(std::time::Duration::from_millis(500));
            match send_result {
                Ok(result) => match result {
                    Ok(confirmation) => {
                        println!("Transaction was accepted");
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
                        error:          Some("internal server error".to_owned()),
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
    
    let pending: Option<Account> = acc_rx.recv_timeout(std::time::Duration::from_millis(100)).expect("must get account info back");
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
    let status: NetworkStatus = rx.recv_timeout(std::time::Duration::from_millis(1000)).expect("must get status back");

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
    let sys = actix::System::new("api-server");
    let server_config = format!("{}:{}", address, port);

    //move is necessary to give closure below ownership
    server::new(move || {
        App::with_state(
            AppState {
                tx_for_state: tx_for_state.clone(),
                contract_address: contract_address.clone(),
                connection_pool: connection_pool.clone(),
            }.clone()
        ) // <- create app with shared state
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

    println!("Started http server: {}", server_config);
    //sys.run();
}