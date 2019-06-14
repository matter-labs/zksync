#![cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))]

use crate::nonce_futures::NonceFutures;
use actix_web::{
    http::Method, middleware, middleware::cors::Cors, server, App, AsyncResponder, Error,
    HttpMessage, HttpRequest, HttpResponse,
};
use models::config::RUNTIME_CONFIG;
use models::plasma::{Account, PublicKey, TransferTx};
use models::{ActionType, NetworkStatus, StateKeeperRequest, TransferTxConfirmation};
use std::sync::mpsc;
use storage::{BlockDetails, ConnectionPool};

use futures::Future;
use std::env;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::prelude::*;
use tokio::timer::Interval;

#[derive(Debug, Serialize, Deserialize)]
struct ApiError {
    error: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TransactionRequest {
    from: u32,
    to: u32,
    amount: u128,
}

#[derive(Debug, Serialize, Deserialize)]
struct TransactionResponse {
    accepted: bool,
    error: Option<String>,
    confirmation: Option<TransferTxConfirmation>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TestnetConfigResponse {
    address: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AccountDetailsResponse {
    pending: Option<Account>,
    verified: Option<Account>,
    committed: Option<Account>,
}

#[derive(Default, Clone)]
struct SharedNetworkStatus(Arc<RwLock<NetworkStatus>>);

impl SharedNetworkStatus {
    fn read(&self) -> NetworkStatus {
        (*self.0.as_ref().read().unwrap()).clone()
    }
}

/// AppState is a collection of records cloned by each thread to shara data between them
#[derive(Clone)]
pub struct AppState {
    tx_for_state: mpsc::Sender<StateKeeperRequest>,
    contract_address: String,
    connection_pool: ConnectionPool,
    nonce_futures: NonceFutures,
    network_status: SharedNetworkStatus,
}

const TIMEOUT: u64 = 500;
const NONCE_ORDER_TIMEOUT: u64 = 800;

fn handle_submit_tx(
    req: &HttpRequest<AppState>,
) -> Box<Future<Item = HttpResponse, Error = Error>> {
    let tx_for_state = req.state().tx_for_state.clone();
    let network_status = req.state().network_status.read();
    let nonce_futures = req.state().nonce_futures.clone();

    req.json()
        .map_err(|e| format!("{}", e)) // convert all errors to String
        .and_then(move |tx: TransferTx| {
            // Rate limit check

            // TODO: check lazy init
            if network_status.outstanding_txs > RUNTIME_CONFIG.max_outstanding_txs {
                return Err(format!("Rate limit exceeded"));
            }

            // Validate tx input

            tx.validate()?;

            // Fetch account

            // TODO: the code below will block the current thread; switch to futures instead
            let (key_tx, key_rx) = mpsc::channel();
            let request = StateKeeperRequest::GetAccount(tx.from, key_tx);
            tx_for_state
                .send(request)
                .expect("must send a new transaction to queue");
            let account = key_rx
                .recv_timeout(std::time::Duration::from_millis(TIMEOUT))
                .map_err(|_| format!("Internal error: timeout on GetAccount"))?;
            let account = account.ok_or(format!("Account not found"))?;

            Ok((tx, account, tx_for_state))
        })
        .and_then(move |(tx, account, tx_for_state)| {
            //println!("account {}: nonce {} received", tx.from, tx.nonce);

            // Wait for nonce

            let future = if account.nonce == tx.nonce {
                nonce_futures.ready(tx.from, tx.nonce)
            } else {
                //println!("account {}: waiting for nonce {}", tx.from, tx.nonce);
                nonce_futures.nonce_await(tx.from, tx.nonce)
            };
            future
                .timeout(Duration::from_millis(NONCE_ORDER_TIMEOUT))
                .map(|_| (tx, account, nonce_futures, tx_for_state))
                .or_else(|e| future::err(format!("Nonce error: {:?}", e)))
        })
        .and_then(move |(mut tx, account, mut nonce_futures, tx_for_state)| {
            //println!("account {}: nonce {} ready", tx.from, tx.nonce);

            // Verify signature

            let pub_key: PublicKey = account.get_pub_key().ok_or(format!("Pubkey expired"))?;
            let verified = tx.verify_sig(&pub_key);
            if !verified {
                let (x, y) = pub_key.0.into_xy();
                println!("Got public key: {:?}, {:?}", x, y);
                println!(
                    "Signature is invalid: (x,y,s) = ({:?},{:?},{:?})",
                    &tx.signature.r_x, &tx.signature.r_y, &tx.signature.s
                );
                return Err(format!("Invalid signature"));
            }

            // Cache public key we just verified against (to skip verifying again in state keeper)

            tx.cached_pub_key = Some(pub_key);

            // Apply tx

            let (add_tx, add_rx) = mpsc::channel();
            let (account, nonce) = (tx.from, tx.nonce);
            tx_for_state
                .send(StateKeeperRequest::AddTransferTx(tx, add_tx))
                .expect("sending to sate keeper failed");
            // TODO: reconsider timeouts
            let confirmation = add_rx
                .recv_timeout(std::time::Duration::from_millis(500))
                .map_err(|_| format!("Internal error: timeout on AddTransferTx"))?
                .map_err(|e| format!("Tx rejected: {:?}", e))?;

            // Notify futures waiting for nonce

            nonce_futures.set_next_nonce(account, nonce + 1);

            // Return response

            let resp = TransactionResponse {
                accepted: true,
                error: None,
                confirmation: Some(confirmation),
            };
            Ok(HttpResponse::Ok().json(resp))
        })
        .or_else(|err: String| {
            let resp = TransactionResponse {
                accepted: false,
                error: Some(err),
                confirmation: None,
            };
            Ok(HttpResponse::Ok().json(resp))
        })
        .responder()
}

use actix_web::Result as ActixResult;

fn handle_get_account_state(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    let tx_for_state = req.state().tx_for_state.clone();
    let pool = req.state().connection_pool.clone();

    let storage = pool.access_storage();
    if storage.is_err() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "rate limit".to_string(),
        }));
    }
    let storage = storage.unwrap();

    // check that something like this exists in state keeper's memory at all
    let account_id_string = req.match_info().get("id");
    if account_id_string.is_none() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "invalid parameters".to_string(),
        }));
    }
    let account_id = account_id_string.unwrap().parse::<u32>();
    if account_id.is_err() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "invalid account_id".to_string(),
        }));
    }

    let (acc_tx, acc_rx) = mpsc::channel();
    let account_id_u32 = account_id.unwrap();
    let request = StateKeeperRequest::GetAccount(account_id_u32, acc_tx);
    tx_for_state
        .send(request)
        .expect("must send a request for an account state");

    let pending: Result<Option<Account>, _> =
        acc_rx.recv_timeout(std::time::Duration::from_millis(TIMEOUT));

    if pending.is_err() {
        println!("API request timeout!");
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "account request timeout".to_string(),
        }));
    }

    let pending = pending.unwrap();
    if pending.is_none() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "non-existing account".to_string(),
        }));
    }

    let committed = storage.last_committed_state_for_account(account_id_u32);
    if let Err(ref err) = &committed {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: format!("db error: {}", err),
        }));
    }

    let verified = storage.last_verified_state_for_account(account_id_u32);
    if let Err(ref err) = &verified {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: format!("db error: {}", err),
        }));
    }

    // QUESTION: why do we need committed here?
    let response = AccountDetailsResponse {
        pending,
        verified: verified.unwrap(),
        committed: committed.unwrap(),
    };

    Ok(HttpResponse::Ok().json(response))
}

fn handle_get_testnet_config(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    let address = req.state().contract_address.clone();
    Ok(HttpResponse::Ok().json(TestnetConfigResponse {
        address: format!("0x{}", address),
    }))
}

// fn handle_get_network_status(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
//     let tx_for_state = req.state().tx_for_state.clone();

//     let (tx, rx) = mpsc::channel();
//     let request = StateKeeperRequest::GetNetworkStatus(tx);
//     tx_for_state.send(request).expect("must send a new transaction to queue");
//     let status: Result<NetworkStatus, _> = rx.recv_timeout(std::time::Duration::from_millis(TIMEOUT));
//     if status.is_err() {
//         return Ok(HttpResponse::Ok().json(ApiError{error: "timeout".to_owned()}));
//     }
//     let status = status.unwrap();

//     let pool = req.state().connection_pool.clone();
//     let storage = pool.access_storage();
//     if storage.is_err() {
//         return Ok(HttpResponse::Ok().json(ApiError{error: "rate limit".to_string()}));
//     }
//     let mut storage = storage.unwrap();

//     // TODO: properly handle failures
//     let last_committed = storage.get_last_committed_block().unwrap_or(0);
//     let last_verified = storage.get_last_verified_block().unwrap_or(0);
//     let outstanding_txs = storage.count_outstanding_proofs(last_verified).unwrap_or(0);

//     let status = NetworkStatus{
//         next_block_at_max: status.next_block_at_max,
//         last_committed,
//         last_verified,
//         outstanding_txs,
//     };

//     Ok(HttpResponse::Ok().json(status))
// }

fn handle_get_network_status(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    let network_status = req.state().network_status.read();
    Ok(HttpResponse::Ok().json(network_status))
}

fn handle_get_account_transactions(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok().json("{}"))
}

fn handle_get_transaction_by_id(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    let pool = req.state().connection_pool.clone();

    let storage = pool.access_storage();
    if storage.is_err() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "rate limit".to_string(),
        }));
    }
    let storage = storage.unwrap();

    let transaction_id_string = req.match_info().get("tx_id");
    if transaction_id_string.is_none() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "invalid parameters".to_string(),
        }));
    }
    let transaction_id = transaction_id_string.unwrap().parse::<u32>();
    if transaction_id.is_err() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "invalid transaction_id".to_string(),
        }));
    }

    let transaction_id_u32 = transaction_id.unwrap();

    let tx = storage.load_transaction_with_id(transaction_id_u32);

    Ok(HttpResponse::Ok().json(tx))
}

fn handle_get_block_transactions(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    let pool = req.state().connection_pool.clone();

    let storage = pool.access_storage();
    if storage.is_err() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "rate limit".to_string(),
        }));
    }
    let storage = storage.unwrap();

    let block_id_string = req.match_info().get("block_id");
    if block_id_string.is_none() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "invalid parameters".to_string(),
        }));
    }
    let block_id = block_id_string.unwrap().parse::<u32>();
    if block_id.is_err() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "invalid block_id".to_string(),
        }));
    }

    let block_id_u32 = block_id.unwrap();

    let result = storage.load_transactions_in_block(block_id_u32);
    match result {
        Ok(txs) => Ok(HttpResponse::Ok().json(txs)),
        Err(err) => Ok(HttpResponse::Ok().json(ApiError {
            error: format!("error: {}", err),
        })),
    }
}

fn handle_get_block_by_id(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    let pool = req.state().connection_pool.clone();

    let storage = pool.access_storage();
    if storage.is_err() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "rate limit".to_string(),
        }));
    }
    let storage = storage.unwrap();

    let block_id_string = req.match_info().get("block_id");
    if block_id_string.is_none() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "invalid parameters".to_string(),
        }));
    }
    let block_id = block_id_string.unwrap().parse::<u32>();
    if block_id.is_err() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "invalid block_id".to_string(),
        }));
    }

    let block_id_u32 = block_id.unwrap();

    // FIXME: no expects in API server db requests, because they might fail temporarily and bring down the server!
    let stored_commit_operation =
        storage.load_stored_op_with_block_number(block_id_u32, ActionType::COMMIT);
    if stored_commit_operation.is_none() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "not found".to_string(),
        }));
    }

    let commit = stored_commit_operation.unwrap();
    let operation = commit.clone().into_op(&storage);
    if let Err(ref err) = &operation {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: format!("db error: {}", err),
        }));
    }
    let operation = operation.unwrap();

    let verify = storage.load_stored_op_with_block_number(block_id_u32, ActionType::VERIFY);

    let response = BlockDetails {
        block_number: commit.block_number as i32,
        new_state_root: format!("0x{}", operation.block.new_root_hash.to_hex()),
        commit_tx_hash: commit.tx_hash,
        verify_tx_hash: verify.as_ref().map_or(None, |op| op.tx_hash.clone()),
        committed_at: commit.created_at,
        verified_at: verify.as_ref().map(|op| op.created_at),
    };

    Ok(HttpResponse::Ok().json(response))
}

struct GetBlocksQuery {
    pub from_block: Option<u32>,
}

// ***
fn handle_get_blocks(
    req: &HttpRequest<AppState>,
) -> Box<Future<Item = HttpResponse, Error = Error>> {
    let pool = req.state().connection_pool.clone();

    let max_block = req
        .query()
        .get("max_block")
        .cloned()
        .unwrap_or("99999999".to_string());
    let limit = req
        .query()
        .get("limit")
        .cloned()
        .unwrap_or("20".to_string());

    req.body()
        .map_err(|err| format!("{}", err))
        .and_then(move |_| {
            let storage = pool
                .access_storage()
                .map_err(|err| format!("db err: {}", err))?;

            let max_block: u32 = max_block
                .parse()
                .map_err(|_| format!("invalid max_block"))?;
            let limit: u32 = limit.parse().map_err(|_| format!("invalid limit"))?;
            if limit > 100 {
                return Err(format!("limit can not exceed 100"));
            }

            let response: Vec<BlockDetails> = storage
                .load_block_range(max_block, limit)
                .map_err(|e| format!("db err: {}", e))?;

            // let commited_stored_operations = storage.load_stored_ops_in_blocks_range(max_block, limit, ActionType::COMMIT);
            // let verified_operations = storage.load_stored_ops_in_blocks_range(max_block, limit, ActionType::VERIFY);

            // let commited_operations: Vec<Operation> = commited_stored_operations.clone().iter()
            // .map(|x| x.clone().into_op(&storage).expect("into_op must work")).collect();

            // // FIXME: oy vey!
            // let response: Vec<BlockDetailsResponse> = commited_stored_operations.into_iter().map(|x|
            //     BlockDetailsResponse {
            //         block_number:        x.clone().block_number as u32,
            //         new_state_root:      commited_operations.iter().find(|&y| y.id.unwrap() == x.id).unwrap().block.new_root_hash.to_hex(),
            //         commit_tx_hash:      x.clone().tx_hash,
            //         verify_tx_hash:      verified_operations.iter().find(|&y| y.id == x.id).map_or(None, |x| x.clone().tx_hash),
            //         committed_at:        None,
            //         verified_at:         None,
            //     }
            // ).collect();
            Ok(HttpResponse::Ok().json(response))

            //Ok(HttpResponse::Ok().json(format!("offset = {:?}", offset)))
        })
        .or_else(|err: String| {
            let resp = TransactionResponse {
                accepted: false,
                error: Some(err),
                confirmation: None,
            };
            Ok(HttpResponse::Ok().json(resp))
        })
        .responder()
}

fn handle_search(req: &HttpRequest<AppState>) -> Box<Future<Item = HttpResponse, Error = Error>> {
    let pool = req.state().connection_pool.clone();
    let query = req.query().get("query").cloned().unwrap_or("".to_string());
    req.body()
        .map_err(|err| format!("{}", err))
        .and_then(move |_| {
            let storage = pool
                .access_storage()
                .map_err(|err| format!("db err: {}", err))?;
            let response: BlockDetails = storage.handle_search(query).ok_or("db err")?;
            Ok(HttpResponse::Ok().json(response))
        })
        .or_else(|err: String| {
            let resp = ApiError { error: err };
            Ok(HttpResponse::Ok().json(resp))
        })
        .responder()
}

fn start_server(state: AppState, bind_to: String) {
    server::new(move || {
        App::with_state(state.clone()) // <- create app with shared state
            .middleware(middleware::Logger::default())
            .middleware(Cors::build().send_wildcard().max_age(3600).finish())
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
                    .resource("/blocks/transactions/{tx_id}", |r| {
                        r.method(Method::GET).f(handle_get_transaction_by_id);
                    })
                    .resource("/blocks/{block_id}/transactions", |r| {
                        r.method(Method::GET).f(handle_get_block_transactions);
                    })
                    .resource("/blocks/{block_id}", |r| {
                        r.method(Method::GET).f(handle_get_block_by_id);
                    })
                    .resource("/blocks", |r| {
                        r.method(Method::GET).f(handle_get_blocks);
                    })
                    .resource("/search", |r| {
                        r.method(Method::GET).f(handle_search);
                    })
            })
    })
    .bind(&bind_to)
    .unwrap()
    .shutdown_timeout(1)
    .start();
}

pub fn start_status_interval(state: AppState) {
    let state_checker = Interval::new(Instant::now(), Duration::from_millis(1000))
        .fold(state.clone(), |state, _instant| {
            let pool = state.connection_pool.clone();
            let storage = pool.access_storage().expect("db failed");

            // TODO: add flag for failure?
            let last_verified = storage.get_last_verified_block().unwrap_or(0);
            let status = NetworkStatus {
                next_block_at_max: None,
                last_committed: storage.get_last_committed_block().unwrap_or(0),
                last_verified,
                total_transactions: storage.count_total_transactions().unwrap_or(0),
                outstanding_txs: storage.count_outstanding_proofs(last_verified).unwrap_or(0),
            };

            // TODO: send StateKeeperRequest::GetNetworkStatus(tx) and get result

            // save status to state
            *state.network_status.0.as_ref().write().unwrap() = status;

            Ok(state)
        })
        .map(|_| ())
        .map_err(|e| panic!("interval errored; err={:?}", e));

    actix::System::with_current(|_| {
        actix::spawn(state_checker);
    });
}

pub fn start_api_server(
    tx_for_state: mpsc::Sender<StateKeeperRequest>,
    connection_pool: ConnectionPool,
) {
    std::thread::Builder::new()
        .name("actix".to_string())
        .spawn(move || {
            env::set_var("RUST_LOG", "actix_web=info");

            let address = env::var("BIND_TO").unwrap_or("127.0.0.1".to_string());
            let port = env::var("PORT").unwrap_or("8080".to_string());
            let bind_to = format!("{}:{}", address, port);

            let sys = actix::System::new("api-server");

            let state = AppState {
                tx_for_state: tx_for_state.clone(),
                contract_address: env::var("CONTRACT_ADDR").expect("CONTRACT_ADDR env missing"),
                connection_pool: connection_pool.clone(),
                nonce_futures: NonceFutures::default(),
                network_status: SharedNetworkStatus::default(),
            };

            start_server(state.clone(), bind_to.clone());
            println!("Started http server at {}", &bind_to);
            start_status_interval(state.clone());
            sys.run();
        });
}
