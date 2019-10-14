#![cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))]


use actix_cors::Cors;
use actix_web::{
    middleware, web, App, HttpRequest, HttpResponse, HttpServer, Result as ActixResult,
};
use models::node::{tx::FranklinTx, Account, AccountId, ExecutedOperations};
use models::{NetworkStatus, StateKeeperRequest};
use std::sync::mpsc;
use storage::{BlockDetails, ConnectionPool, StorageProcessor, TxReceiptResponse};

use crate::ThreadPanicNotify;
use failure::format_err;
use futures::{sync::oneshot, Future};
use models::node::AccountAddress;
use std::collections::BTreeMap;
use std::convert::TryInto;
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

impl AppState {
    fn access_storage(&self) -> ActixResult<StorageProcessor> {
        self.connection_pool
            .access_storage()
            .map_err(|_| HttpResponse::RequestTimeout().finish().into())
    }
}

#[derive(Debug)]
struct OperationNotifier {
    watched_txs: BTreeMap<Vec<u8>, Vec<oneshot::Sender<TxReceiptResponse>>>,
}

impl OperationNotifier {
    fn sub_tx_update(&mut self, tx_hash: Vec<u8>, recv_notify: oneshot::Sender<TxReceiptResponse>) {
        let mut subs = self.watched_txs.remove(&tx_hash).unwrap_or_default();
        subs.push(recv_notify);
        self.watched_txs.insert(tx_hash, subs);
    }

    fn poll_updates_db(&mut self, conn_pool: ConnectionPool) {
        let storage = if let Ok(storage) = conn_pool.access_storage() {
            storage
        } else {
            return;
        };

        self.watched_txs = std::mem::replace(&mut self.watched_txs, BTreeMap::default())
            .into_iter()
            .filter_map(|(k, v)| match storage.tx_receipt(&k) {
                Ok(Some(receipt)) => {
                    for subs in v.into_iter() {
                        subs.send(receipt.clone()).unwrap_or_default();
                    }
                    None
                }
                _ => Some((k, v)),
            })
            .collect();
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct NewTxResponse {
    hash: String,
    err: Option<String>,
}

fn handle_submit_tx(
    data: web::Data<AppState>,
    tx: web::Json<FranklinTx>,
) -> ActixResult<HttpResponse> {
    let pool = data.connection_pool.clone();
    let hash = hex::encode(tx.hash().as_ref());
    let storage = data.access_storage()?;

    let tx_add_result = storage
        .mempool_add_tx(&tx)
        .map_err(|_| HttpResponse::InternalServerError().finish())?;

    Ok(HttpResponse::Ok().json(NewTxResponse {
        hash,
        err: tx_add_result.err().map(|e| e.to_string()),
    }))
}

#[derive(Debug, Serialize)]
struct AccountStateResponce {
    // None if account is not created yet.
    id: Option<AccountId>,
    commited: Account,
    verified: Account,
    pending_txs: Vec<FranklinTx>,
}
fn handle_get_account_state(
    data: web::Data<AppState>,
    account_address: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let address = AccountAddress::from_hex(&account_address)
        .map_err(|_| HttpResponse::BadRequest().finish())?;

    let storage = data.access_storage()?;

    let (id, verified, commited) = {
        storage
            .account_state_by_address(&address)
            .map_err(|_| HttpResponse::InternalServerError().finish())?
    };

    let pending_txs = {
        storage
            .get_pending_txs(&address)
            .map_err(|_| HttpResponse::InternalServerError().finish())?
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

fn handle_get_tokens(data: web::Data<AppState>) -> ActixResult<HttpResponse> {
    let storage = data.access_storage()?;
    let tokens = storage
        .load_tokens()
        .map_err(|_| HttpResponse::InternalServerError().finish())?;
    Ok(HttpResponse::Ok().json(tokens))
}

fn handle_get_testnet_config(data: web::Data<AppState>) -> HttpResponse {
    let address = data.contract_address.clone();
    HttpResponse::Ok().json(TestnetConfigResponse { address })
}

fn handle_get_account_transactions(
    data: web::Data<AppState>,
    account_address: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let address = AccountAddress::from_hex(&account_address)
        .map_err(|_| HttpResponse::BadRequest().finish())?;
    let storage = data
        .connection_pool
        .access_storage()
        .map_err(|_| HttpResponse::RequestTimeout().finish())?;
    let txs = storage
        .get_account_transactions(&address)
        .map_err(|_| HttpResponse::InternalServerError().finish())?;
    Ok(HttpResponse::Ok().json(txs))
}

fn handle_get_executed_transaction_by_hash(
    data: web::Data<AppState>,
    tx_hash: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let storage = data.access_storage()?;

    let tx_hash: [u8; 32] = {
        if tx_hash.len() != 32 * 2 {
            return Err(HttpResponse::BadRequest().finish().into());
        }
        let vec = hex::decode(tx_hash.as_ref()).map_err(|_| HttpResponse::BadRequest().finish())?;
        vec.as_slice().try_into().unwrap()
    };
    let receipt = storage
        .tx_receipt(&tx_hash)
        .map_err(|_| HttpResponse::InternalServerError().finish())?;
    Ok(HttpResponse::Ok().json(receipt))
}

fn handle_get_network_status(data: web::Data<AppState>) -> HttpResponse {
    HttpResponse::Ok().json(data.network_status.read())
}

#[derive(Deserialize)]
struct HandleBlocksQuery {
    max_block: Option<u32>,
    limit: Option<u32>,
}

fn handle_get_blocks(
    data: web::Data<AppState>,
    query: web::Query<HandleBlocksQuery>,
) -> ActixResult<HttpResponse> {
    let max_block = query.max_block.unwrap_or(999999999);
    let limit = query.limit.unwrap_or(20);
    if limit > 100 {
        return Err(HttpResponse::BadRequest().finish().into());
    }
    let storage = data.access_storage()?;

    let resp = storage
        .load_block_range(max_block, limit)
        .map_err(|_| HttpResponse::InternalServerError().finish())?;
    Ok(HttpResponse::Ok().json(resp))
}

fn handle_get_block_by_id(
    data: web::Data<AppState>,
    block_id: web::Path<u32>,
) -> ActixResult<HttpResponse> {
    let storage = data.access_storage()?;
    let mut blocks = storage
        .load_block_range(block_id.into_inner(), 1)
        .map_err(|_| HttpResponse::InternalServerError().finish())?;
    if let Some(block) = blocks.pop() {
        Ok(HttpResponse::Ok().json(block))
    } else {
        Err(HttpResponse::NotFound().finish().into())
    }
}

fn handle_get_block_transactions(
    data: web::Data<AppState>,
    block_id: web::Path<u32>,
) -> ActixResult<HttpResponse> {
    let storage = data.access_storage()?;
    let ops = storage
        .get_block_executed_ops(block_id.into_inner())
        .map_err(|_| HttpResponse::InternalServerError().finish())?;
    let not_failed_ops = ops
        .into_iter()
        .filter(|op| match op {
            ExecutedOperations::Tx(tx) => tx.op.is_some(),
            _ => true,
        })
        .collect::<Vec<_>>();
    Ok(HttpResponse::Ok().json(not_failed_ops))
}

fn handle_get_transaction_by_id(
    data: web::Data<AppState>,
    req: web::Path<(u32, usize)>,
) -> ActixResult<HttpResponse> {
    let (block_id, tx_id) = req.into_inner();

    let storage = data.access_storage()?;
    let ops = storage
        .get_block_executed_ops(block_id)
        .map_err(|_| HttpResponse::InternalServerError().finish())?;
    if let Some(op) = ops.get(tx_id) {
        Ok(HttpResponse::Ok().json2(op))
    } else {
        Err(HttpResponse::NotFound().finish().into())
    }
}

#[derive(Deserialize)]
struct BlockSearchQuery {
    query: String,
}

fn handle_search(
    data: web::Data<AppState>,
    query: web::Query<BlockSearchQuery>,
) -> ActixResult<HttpResponse> {
    let storage = data.access_storage()?;
    let result = storage.handle_search(query.into_inner().query);
    if let Some(block) = result {
        Ok(HttpResponse::Ok().json(block))
    } else {
        Err(HttpResponse::NotFound().finish().into())
    }
}

#[derive(Debug, Clone, Serialize)]
struct PriorityOpStatus {
    executed: bool,
    block: Option<i64>,
}

fn handle_get_priority_op_status(
    data: web::Data<AppState>,
    priority_op_id: web::Path<u32>,
) -> ActixResult<HttpResponse> {
    let storage = data.access_storage()?;
    let priority_op = storage
        .get_executed_priority_op(priority_op_id.into_inner())
        .map_err(|_| HttpResponse::InternalServerError().finish())?;
    Ok(HttpResponse::Ok().json(PriorityOpStatus {
        executed: priority_op.is_some(),
        block: priority_op.map(|op| op.block_number),
    }))
}

fn start_server(state: AppState, bind_to: String) {
    HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .wrap(middleware::Logger::default())
            .wrap(Cors::new().send_wildcard().max_age(3600))
            .service(
                web::scope("/api/v0.1")
                    .route("/testnet_config", web::get().to(handle_get_testnet_config))
                    .route("/status", web::get().to(handle_get_network_status))
                    .route("/submit_tx", web::post().to(handle_submit_tx))
                    .route(
                        "/account/{address}",
                        web::get().to(handle_get_account_state),
                    )
                    .route("/tokens", web::get().to(handle_get_tokens))
                    .route(
                        "/account/{id}/transactions",
                        web::get().to(handle_get_account_transactions),
                    )
                    .route(
                        "/transactions/{tx_hash}",
                        web::get().to(handle_get_executed_transaction_by_hash),
                    )
                    .route(
                        "/blocks/{block_id}/transactions/{tx_id}",
                        web::get().to(handle_get_transaction_by_id),
                    )
                    .route(
                        "/blocks/{block_id}/transactions",
                        web::get().to(handle_get_block_transactions),
                    )
                    .route("/blocks/{block_id}", web::get().to(handle_get_block_by_id))
                    .route("/blocks", web::get().to(handle_get_blocks))
                    .route(
                        "/priority_op/{priority_op_id}",
                        web::get().to(handle_get_priority_op_status),
                    )
                    .route("/search", web::get().to(handle_search)),
            )
    })
    .client_timeout(0)
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

    //    actix::System::with_current(|_| {
    //        actix::spawn(state_checker);
    //    });
}

pub fn start_api_server(
    tx_for_state: mpsc::Sender<StateKeeperRequest>,
    connection_pool: ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
) {
    std::thread::Builder::new()
        .name("actix".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify);

            let address = env::var("BIND_TO").unwrap_or_else(|_| "127.0.0.1".to_string());
            let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
            let bind_to = format!("{}:{}", address, port);

            let runtime = actix_rt::System::new("api-server");

            let state = AppState {
                tx_for_state: tx_for_state.clone(),
                contract_address: env::var("CONTRACT_ADDR").expect("CONTRACT_ADDR env missing"),
                connection_pool: connection_pool.clone(),
                network_status: SharedNetworkStatus::default(),
            };

            start_server(state.clone(), bind_to.clone());
            info!("Started http server at {}", &bind_to);
            start_status_interval(state.clone());
            runtime.run().unwrap_or_default();
        })
        .expect("Api server thread");
}
