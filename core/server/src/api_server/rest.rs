use actix_cors::Cors;
use actix_web::{
    middleware,
    web::{self},
    App, HttpResponse, HttpServer, Result as ActixResult,
};
use models::node::{Account, AccountAddress, AccountId, ExecutedOperations, FranklinTx};
use models::NetworkStatus;
use std::sync::mpsc;
use storage::{ConnectionPool, StorageProcessor};

use crate::ThreadPanicNotify;
use futures::{Future, Stream};
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::timer::Interval;
use web3::types::H160;

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
struct AppState {
    connection_pool: ConnectionPool,
    network_status: SharedNetworkStatus,
    contract_address: String,
}

impl AppState {
    fn access_storage(&self) -> ActixResult<StorageProcessor> {
        self.connection_pool
            .access_storage()
            .map_err(|_| HttpResponse::RequestTimeout().finish().into())
    }

    // Spawns future updating SharedNetworkStatus in the current `actix::System`
    fn spawn_network_status_updater(&self) {
        let state_checker = Interval::new(Instant::now(), Duration::from_millis(1000))
            .fold(self.clone(), |state, _instant| {
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
}
#[derive(Debug, Serialize)]
struct TestnetConfigResponse {
    address: String,
}

// TODO: remove, JSON-rpc get contract should be used instead
fn handle_get_testnet_config(data: web::Data<AppState>) -> ActixResult<HttpResponse> {
    let address = data.contract_address.clone();
    Ok(HttpResponse::Ok().json(TestnetConfigResponse { address }))
}

fn handle_get_network_status(data: web::Data<AppState>) -> ActixResult<HttpResponse> {
    let network_status = data.network_status.read();
    Ok(HttpResponse::Ok().json(network_status))
}

#[derive(Debug, Serialize, Deserialize)]
struct NewTxResponse {
    hash: String,
}

fn handle_submit_tx(
    data: web::Data<AppState>,
    req: web::Json<FranklinTx>,
) -> ActixResult<HttpResponse> {
    let storage = data.access_storage()?;

    let tx_add_result = storage
        .mempool_add_tx(&req)
        .map_err(|_| HttpResponse::InternalServerError().finish())?;

    if let Err(e) = tx_add_result {
        Err(HttpResponse::NotAcceptable().body(e.to_string()).into())
    } else {
        Ok(HttpResponse::Ok().json(NewTxResponse {
            hash: req.hash().to_hex(),
        }))
    }
}

#[derive(Debug, Serialize)]
struct AccountStateResponse {
    // None if account is not created yet.
    id: Option<AccountId>,
    commited: Account,
    verified: Account,
    pending_txs: Vec<FranklinTx>,
}

fn handle_get_account_state(
    data: web::Data<AppState>,
    account_address: web::Path<AccountAddress>,
) -> ActixResult<HttpResponse> {
    let storage = data.access_storage()?;

    let (id, verified, commited) = {
        let stored_account_state = storage
            .account_state_by_address(&account_address)
            .map_err(|_| HttpResponse::InternalServerError().finish())?;

        let empty_state = |address: &AccountAddress| {
            let mut acc = Account::default();
            acc.address = address.clone();
            acc
        };

        let id = stored_account_state.committed.as_ref().map(|(id, _)| *id);
        let committed = stored_account_state
            .committed
            .map(|(_, acc)| acc)
            .unwrap_or_else(|| empty_state(&account_address));
        let verified = stored_account_state
            .verified
            .map(|(_, acc)| acc)
            .unwrap_or_else(|| empty_state(&account_address));

        (id, verified, committed)
    };

    let pending_txs = storage
        .get_pending_txs(&account_address)
        .map_err(|_| HttpResponse::InternalServerError().finish())?;

    let res = AccountStateResponse {
        id,
        commited,
        verified,
        pending_txs,
    };

    Ok(HttpResponse::Ok().json(res))
}

fn handle_get_tokens(data: web::Data<AppState>) -> ActixResult<HttpResponse> {
    let storage = data.access_storage()?;
    let tokens = storage
        .load_tokens()
        .map_err(|_| HttpResponse::InternalServerError().finish())?;

    let mut vec_tokens = tokens.values().cloned().collect::<Vec<_>>();
    vec_tokens.sort_by_key(|t| t.id);

    Ok(HttpResponse::Ok().json(vec_tokens))
}

fn handle_get_account_transactions(
    data: web::Data<AppState>,
    address: web::Path<AccountAddress>,
) -> ActixResult<HttpResponse> {
    let storage = data.access_storage()?;
    let txs = storage
        .get_account_transactions(&address)
        .map_err(|_| HttpResponse::InternalServerError().finish())?;
    Ok(HttpResponse::Ok().json(txs))
}

fn handle_get_account_transactions_history(
    data: web::Data<AppState>,
    request_path: web::Path<(AccountAddress, i64, i64)>,
) -> ActixResult<HttpResponse> {
    let (address, offset, limit) = request_path.into_inner();

    const MAX_LIMIT: i64 = 100;
    if limit > MAX_LIMIT {
        return Err(HttpResponse::BadRequest().finish().into());
    }

    let storage = data.access_storage()?;

    let res = storage
        .get_account_transactions_history(&address, offset, limit)
        .map_err(|_| HttpResponse::InternalServerError().finish())?;

    Ok(HttpResponse::Ok().json(res))
}

fn handle_get_executed_transaction_by_hash(
    data: web::Data<AppState>,
    tx_hash_hex: web::Path<String>,
) -> ActixResult<HttpResponse> {
    if tx_hash_hex.len() < 2 {
        return Err(HttpResponse::BadRequest().finish().into());
    }
    let transaction_hash = hex::decode(&tx_hash_hex.into_inner()[2..])
        .map_err(|_| HttpResponse::BadRequest().finish())?;

    let storage = data.access_storage()?;
    if let Ok(tx) = storage.tx_receipt(transaction_hash.as_slice()) {
        Ok(HttpResponse::Ok().json(tx))
    } else {
        Ok(HttpResponse::Ok().json(()))
    }
}

fn handle_get_tx_by_hash(
    data: web::Data<AppState>,
    hash_hex_with_0x: web::Path<String>,
) -> ActixResult<HttpResponse> {
    if hash_hex_with_0x.len() < 2 {
        return Err(HttpResponse::BadRequest().finish().into());
    }
    let hash = hex::decode(&hash_hex_with_0x.into_inner()[2..])
        .map_err(|_| HttpResponse::BadRequest().finish())?;

    let storage = data.access_storage()?;

    let res = storage
        .get_tx_by_hash(hash.as_slice())
        .map_err(|_| HttpResponse::InternalServerError().finish())?;

    Ok(HttpResponse::Ok().json(res))
}

fn handle_get_priority_op_receipt(
    data: web::Data<AppState>,
    id: web::Path<i64>,
) -> ActixResult<HttpResponse> {
    let storage = data.access_storage()?;

    let res = storage
        .get_priority_op_receipt(id.into_inner())
        .map_err(|_| HttpResponse::InternalServerError().finish())?;

    Ok(HttpResponse::Ok().json(res))
}

fn handle_get_transaction_by_id(
    data: web::Data<AppState>,
    path: web::Path<(u32, u32)>,
) -> ActixResult<HttpResponse> {
    let (block_id, tx_id) = path.into_inner();

    let storage = data.access_storage()?;

    let executed_ops = storage
        .get_block_executed_ops(block_id)
        .map_err(|_| HttpResponse::InternalServerError().finish())?;

    if let Some(exec_op) = executed_ops.get(tx_id as usize) {
        Ok(HttpResponse::Ok().json(exec_op))
    } else {
        Err(HttpResponse::NotFound().finish().into())
    }
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
    let max_block = query.max_block.unwrap_or(999_999_999);
    let limit = query.limit.unwrap_or(20);
    if limit > 100 {
        return Err(HttpResponse::BadRequest().finish().into());
    }
    let storage = data.access_storage()?;

    let resp = storage.load_block_range(max_block, limit).map_err(|e| {
        warn!("handle_get_blocks db fail: {}", e);
        HttpResponse::InternalServerError().finish()
    })?;
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
    path: web::Path<u32>,
) -> ActixResult<HttpResponse> {
    let block_id = path.into_inner();

    let storage = data.access_storage()?;

    let executed_ops = storage
        .get_block_executed_ops(block_id)
        .map_err(|_| HttpResponse::InternalServerError().finish())?
        .into_iter()
        .filter(|op| match op {
            ExecutedOperations::Tx(tx) => tx.op.is_some(),
            _ => true,
        })
        .collect::<Vec<_>>();

    #[derive(Serialize)]
    struct ExecutedOperationWithHash {
        op: ExecutedOperations,
        tx_hash: String,
    };

    let executed_ops_with_hashes = executed_ops
        .into_iter()
        .map(|op| {
            let tx_hash = match &op {
                ExecutedOperations::Tx(tx) => tx.tx.hash().as_ref().to_vec(),
                ExecutedOperations::PriorityOp(tx) => tx.priority_op.eth_hash.clone(),
            };

            let tx_hash = format!("0x{}", hex::encode(&tx_hash));

            ExecutedOperationWithHash { op, tx_hash }
        })
        .collect::<Vec<_>>();

    Ok(HttpResponse::Ok().json(executed_ops_with_hashes))
}

#[derive(Deserialize)]
struct BlockSearchQuery {
    query: String,
}

fn handle_block_search(
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

fn start_server(state: AppState, bind_to: SocketAddr) {
    HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .wrap(middleware::Logger::default())
            .wrap(Cors::new().send_wildcard().max_age(3600))
            .service(
                web::scope("/api/v0.1")
                    .route(
                        "/blocks/{block_id}/transactions",
                        web::get().to(handle_get_block_transactions),
                    )
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
                        "/account/{address}/history/{offset}/{limit}",
                        web::get().to(handle_get_account_transactions_history),
                    )
                    .route(
                        "/transactions/{tx_hash}",
                        web::get().to(handle_get_executed_transaction_by_hash),
                    )
                    .route(
                        "/transactions_all/{tx_hash}",
                        web::get().to(handle_get_tx_by_hash),
                    )
                    .route(
                        "/priority_operations/{pq_id}/",
                        web::get().to(handle_get_priority_op_receipt),
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
                    .route("/search", web::get().to(handle_block_search)),
            )
            // Endpoint needed for js isReachable
            .route(
                "/favicon.ico",
                web::get().to(|| HttpResponse::Ok().finish()),
            )
    })
    .bind(bind_to)
    .unwrap()
    .shutdown_timeout(1)
    .start();
}

/// Start HTTP REST API
pub(super) fn start_server_thread_detached(
    connection_pool: ConnectionPool,
    listen_addr: SocketAddr,
    contract_address: H160,
    panic_notify: mpsc::Sender<bool>,
) {
    std::thread::Builder::new()
        .name("actix-rest-api".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify);

            let runtime = actix_rt::System::new("api-server");

            let state = AppState {
                connection_pool,
                network_status: SharedNetworkStatus::default(),
                contract_address: format!("{}", contract_address),
            };
            state.spawn_network_status_updater();

            start_server(state, listen_addr);
            runtime.run().unwrap_or_default();
        })
        .expect("Api server thread");
}
