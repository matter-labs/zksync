#![cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))]

use actix_web::{
    http::Method, middleware, middleware::cors::Cors, server, App, AsyncResponder, Body, Error,
    HttpMessage, HttpRequest, HttpResponse,
};
use models::node::{tx::FranklinTx, Account, AccountId};
use models::{ActionType, NetworkStatus, StateKeeperRequest};
use std::sync::mpsc;
use storage::{BlockDetails, ConnectionPool};

use crate::ThreadPanicNotify;
use actix_web::Result as ActixResult;
use failure::format_err;
use futures::Future;
use models::node::AccountAddress;
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
}

#[derive(Debug, Serialize, Deserialize)]
struct TestnetConfigResponse {
    address: String,
}

#[derive(Default, Clone)]
struct SharedNetworkStatus(Arc<RwLock<NetworkStatus>>);

impl SharedNetworkStatus {
    #[allow(dead_code)]
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
    network_status: SharedNetworkStatus,
}

//let storage = pool
//.access_mempool()
//.map_err(|_| HttpResponse::InternalServerError().body(Body::Empty));
//let storage_query_result = storage.and_then(|storage| {
//storage.add_tx(tx)
//.map(|tx_add_result| {
//match tx_add_result {
//Ok(_) => HttpResponse::Ok().body(Body::Empty),
//Err(e) => HttpResponse::BadRequest().json(e),
//}
//})
//.map_err(|_| HttpResponse::InternalServerError().body(Body::Empty))
//});

#[derive(Debug, Serialize, Deserialize)]
struct NewTxResponse {
    hash: String,
    err: Option<String>,
}

fn handle_submit_tx(
    req: &HttpRequest<AppState>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let pool = req.state().connection_pool.clone();

    req.json()
        .map_err(|e| format!("{}", e)) // convert all errors to String
        .and_then(move |tx: FranklinTx| {
            // Rate limit check
            let tx_hash = hex::encode(&tx.hash());

            let storage = pool
                .access_storage()
                .map_err(|e| format!("db error: {}", e))?;
            let response = storage
                .mempool_add_tx(&tx)
                .map(|tx_add_result| {
                    let resp = match tx_add_result {
                        Ok(_) => NewTxResponse {
                            hash: tx_hash.clone(),
                            err: None,
                        },
                        Err(e) => NewTxResponse {
                            hash: tx_hash.clone(),
                            err: Some(format!("{}", e)),
                        },
                    };
                    HttpResponse::Ok().json(resp)
                })
                .map_err(|e| {
                    let resp = NewTxResponse {
                        hash: tx_hash.clone(),
                        err: Some(format!("mempool_error: {}", e)),
                    };
                    HttpResponse::Ok().json(resp)
                });
            let response = match response {
                Ok(ok) => ok,
                Err(err) => err,
            };
            Ok(response)
        })
        .or_else(|err: String| Ok(HttpResponse::InternalServerError().json(err)))
        .responder()
}

#[derive(Debug, Serialize)]
struct AccountStateResponce {
    // None if account is not created yet.
    id: Option<AccountId>,
    commited: Account,
    verified: Account,
    pending_txs: Vec<FranklinTx>,
}
fn handle_get_account_state(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    // check that something like this exists in state keeper's memory at all
    let account_address = req.match_info().get("address");
    let address = if let Some(account_address) = account_address {
        AccountAddress::from_hex(account_address)?
    } else {
        return Err(format_err!("Invalid parameters").into());
    };

    let pool = req.state().connection_pool.clone();

    let (id, verified, commited) = {
        let storage = pool
            .access_storage()
            .map_err(|_| HttpResponse::InternalServerError().body(Body::Empty));
        let storage_query_result = storage.and_then(|storage| {
            storage
                .account_state_by_address(&address)
                .map_err(|_| HttpResponse::InternalServerError().body(Body::Empty))
        });

        match storage_query_result {
            Ok((id, verified, commited)) => (id, verified, commited),
            Err(e) => {
                return Ok(e);
            }
        }
    };

    let pending_txs = {
        let storage = pool
            .access_storage()
            .map_err(|_| HttpResponse::InternalServerError().body(Body::Empty));
        let storage_query_result = storage.and_then(|storage| {
            storage
                .get_pending_txs(&address)
                .map_err(|_| HttpResponse::InternalServerError().body(Body::Empty))
        });
        match storage_query_result {
            Ok(txs) => (txs),
            Err(e) => {
                return Ok(e);
            }
        }
    };

    let empty_state = |address: &AccountAddress| {
        let mut acc = Account::default();
        acc.address = address.clone();
        acc
    };

    let res = AccountStateResponce {
        id,
        commited: commited.unwrap_or_else(|| empty_state(&address)),
        verified: verified.unwrap_or_else(|| empty_state(&address)),
        pending_txs,
    };

    Ok(HttpResponse::Ok().json(res))
}

fn handle_get_tokens(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    let pool = req.state().connection_pool.clone();
    let storage = pool.access_storage();
    if storage.is_err() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "rate limit".to_string(),
        }));
    }
    let storage = storage.unwrap();
    let tokens = match storage
        .load_tokens()
        .map_err(|_| HttpResponse::InternalServerError().body(Body::Empty))
    {
        Ok(tokens) => tokens,
        Err(e) => {
            return Ok(e);
        }
    };

    Ok(HttpResponse::Ok().json(tokens))
}

fn handle_get_testnet_config(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    let address = req.state().contract_address.clone();
    Ok(HttpResponse::Ok().json(TestnetConfigResponse { address }))
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

fn handle_get_account_transactions(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    let account_address = req.match_info().get("id");
    let address = if let Some(account_address) = account_address {
        AccountAddress::from_hex(account_address)?
    } else {
        return Err(format_err!("Invalid parameters").into());
    };

    let pool = req.state().connection_pool.clone();

    let storage = pool
        .access_storage()
        .map_err(|_| HttpResponse::InternalServerError().body(Body::Empty));
    let storage_query_result = storage.and_then(|storage| {
        storage
            .get_account_transactions(&address)
            .map_err(|_| HttpResponse::InternalServerError().body(Body::Empty))
    });
    
    let res = match storage_query_result {
        Ok(txs) => txs,
        Err(e) => {
            return Ok(e);
        }
    };

    Ok(HttpResponse::Ok().json(res))
}

//fn handle_get_transaction_by_id(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
//    let pool = req.state().connection_pool.clone();
//
//    let storage = pool.access_storage();
//    if storage.is_err() {
//        return Ok(HttpResponse::Ok().json(ApiError {
//            error: "rate limit".to_string(),
//        }));
//    }
//    let storage = storage.unwrap();
//
//    let transaction_id_string = req.match_info().get("tx_id");
//    if transaction_id_string.is_none() {
//        return Ok(HttpResponse::Ok().json(ApiError {
//            error: "invalid parameters".to_string(),
//        }));
//    }
//    let transaction_id = transaction_id_string.unwrap().parse::<u32>();
//    if transaction_id.is_err() {
//        return Ok(HttpResponse::Ok().json(ApiError {
//            error: "invalid transaction_id".to_string(),
//        }));
//    }
//
//    let transaction_id_u32 = transaction_id.unwrap();
//
//    let tx = storage.load_transaction_with_id(transaction_id_u32);
//
//    Ok(HttpResponse::Ok().json(tx))
//}

fn handle_get_executed_transaction_by_hash(
    req: &HttpRequest<AppState>,
) -> ActixResult<HttpResponse> {
    let pool = req.state().connection_pool.clone();

    let storage = pool.access_storage();
    if storage.is_err() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "rate limit".to_string(),
        }));
    }
    let storage = storage.unwrap();

    let transaction_hash_string = req.match_info().get("tx_hash");
    if transaction_hash_string.is_none() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "invalid parameters".to_string(),
        }));
    }
    let transaction_hash_string = transaction_hash_string.unwrap();
    let transaction_hash = hex::decode(transaction_hash_string).unwrap();

    if let Ok(tx) = storage.tx_receipt(transaction_hash.as_slice()) {
        Ok(HttpResponse::Ok().json(tx))
    } else {
        Ok(HttpResponse::Ok().json(()))
    }
}

fn handle_get_network_status(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    let network_status = req.state().network_status.read();
    Ok(HttpResponse::Ok().json(network_status))
}

fn handle_get_block_by_id(_req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    unimplemented!()
    //    let pool = req.state().connection_pool.clone();
    //
    //    let storage = pool.access_storage();
    //    if storage.is_err() {
    //        return Ok(HttpResponse::Ok().json(ApiError {
    //            error: "rate limit".to_string(),
    //        }));
    //    }
    //    let storage = storage.unwrap();
    //
    //    let block_id_string = req.match_info().get("block_id");
    //    if block_id_string.is_none() {
    //        return Ok(HttpResponse::Ok().json(ApiError {
    //            error: "invalid parameters".to_string(),
    //        }));
    //    }
    //    let block_id = block_id_string.unwrap().parse::<u32>();
    //    if block_id.is_err() {
    //        return Ok(HttpResponse::Ok().json(ApiError {
    //            error: "invalid block_id".to_string(),
    //        }));
    //    }
    //
    //    let block_id_u32 = block_id.unwrap();
    //
    //    let stored_commit_operation =
    //        storage.load_stored_op_with_block_number(block_id_u32, ActionType::COMMIT);
    //    if stored_commit_operation.is_none() {
    //        return Ok(HttpResponse::Ok().json(ApiError {
    //            error: "not found".to_string(),
    //        }));
    //    }
    //
    //    let commit = stored_commit_operation.unwrap();
    //    let operation = commit.clone().into_op(&storage);
    //    if let Err(ref err) = &operation {
    //        return Ok(HttpResponse::Ok().json(ApiError {
    //            error: format!("db error: {}", err),
    //        }));
    //    }
    //    let operation = operation.unwrap();
    //
    //    let verify = storage.load_stored_op_with_block_number(block_id_u32, ActionType::VERIFY);
    //
    //    let response = BlockDetails {
    //        block_number: commit.block_number as i64,
    //        new_state_root: format!("0x{}", operation.block.new_root_hash.to_hex()),
    //        commit_tx_hash: commit.tx_hash,
    //        verify_tx_hash: verify.as_ref().and_then(|op| op.tx_hash.clone()),
    //        committed_at: commit.created_at,
    //        verified_at: verify.as_ref().map(|op| op.created_at),
    //    };
    //
    //    Ok(HttpResponse::Ok().json(response))
}

fn handle_get_blocks(
    req: &HttpRequest<AppState>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let pool = req.state().connection_pool.clone();

    let max_block = req
        .query()
        .get("max_block")
        .cloned()
        .unwrap_or_else(|| "99999999".to_string());
    let limit = req
        .query()
        .get("limit")
        .cloned()
        .unwrap_or_else(|| "20".to_string());

    req.body()
        .map_err(|err| format!("{}", err))
        .and_then(move |_| {
            let storage = pool
                .access_storage()
                .map_err(|err| format!("db err: {}", err))?;

            let max_block: u32 = max_block
                .parse()
                .map_err(|_| "invalid max_block".to_string())?;
            let limit: u32 = limit.parse().map_err(|_| "invalid limit".to_string())?;
            if limit > 100 {
                return Err("limit can not exceed 100".to_string());
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
            };
            Ok(HttpResponse::Ok().json(resp))
        })
        .responder()
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

    let stored_commit_operation =
        storage.load_stored_op_with_block_number(block_id_u32, ActionType::COMMIT);
    if stored_commit_operation.is_none() {
        return Ok(HttpResponse::Ok().json(ApiError {
            error: "not found".to_string(),
        }));
    }

    let commit = stored_commit_operation.unwrap();

    let txs = commit.get_txs();
    match txs {
        Ok(res) => Ok(HttpResponse::Ok().json(res)),
        Err(err) => Ok(HttpResponse::Ok().json(ApiError {
            error: format!("error: {}", err),
        })),
    }
}

//#[allow(dead_code)]
//struct GetBlocksQuery {
//    pub from_block: Option<u32>,
//}

fn handle_search(
    req: &HttpRequest<AppState>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let pool = req.state().connection_pool.clone();
    let query = req.query().get("query").cloned().unwrap_or_default();
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
                    .resource("/account/{address}", |r| {
                        r.method(Method::GET).f(handle_get_account_state);
                    })
                    .resource("/tokens", |r| {
                        r.method(Method::GET).f(handle_get_tokens);
                    })
                    .resource("/account/{id}/transactions", |r| {
                        r.method(Method::GET).f(handle_get_account_transactions);
                    })
                    // .resource("/blocks/transactions/{tx_id}", |r| {
                    //     r.method(Method::GET).f(handle_get_transaction_by_id);
                    // })
                    .resource("/transactions/{tx_hash}", |r| {
                        r.method(Method::GET)
                            .f(handle_get_executed_transaction_by_hash);
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
    panic_notify: mpsc::Sender<bool>,
) {
    std::thread::Builder::new()
        .name("actix".to_string())
        .spawn(move || {
            env::set_var("RUST_LOG", "actix_web=info");
            let _panic_sentinel = ThreadPanicNotify(panic_notify);

            let address = env::var("BIND_TO").unwrap_or_else(|_| "127.0.0.1".to_string());
            let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
            let bind_to = format!("{}:{}", address, port);

            let sys = actix::System::new("api-server");

            let state = AppState {
                tx_for_state: tx_for_state.clone(),
                contract_address: env::var("CONTRACT_ADDR").expect("CONTRACT_ADDR env missing"),
                connection_pool: connection_pool.clone(),
                network_status: SharedNetworkStatus::default(),
            };

            start_server(state.clone(), bind_to.clone());
            info!("Started http server at {}", &bind_to);
            start_status_interval(state.clone());
            sys.run();
        })
        .expect("Api server thread");
}
