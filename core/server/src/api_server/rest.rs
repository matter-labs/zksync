use crate::mempool::MempoolRequest;
use crate::utils::shared_lru_cache::SharedLruCache;
use actix_cors::Cors;
use actix_web::{
    middleware,
    web::{self},
    App, HttpResponse, HttpServer, Result as ActixResult,
};
use futures::channel::mpsc;
use models::config_options::ThreadPanicNotify;
use models::node::{Account, AccountId, Address, ExecutedOperations, PubKeyHash};
use models::NetworkStatus;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use storage::chain::block::records::BlockDetails;
use storage::chain::operations_ext::records::{PriorityOpReceiptResponse, TxReceiptResponse};
use storage::{ConnectionPool, StorageProcessor};
use tokio::{runtime::Runtime, time};
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
    cache_of_transaction_receipts: SharedLruCache<Vec<u8>, TxReceiptResponse>,
    cache_of_priority_op_receipts: SharedLruCache<u32, PriorityOpReceiptResponse>,
    cache_of_block_executed_ops: SharedLruCache<u32, Vec<ExecutedOperations>>,
    cache_of_blocks_info: SharedLruCache<u32, BlockDetails>,
    cache_blocks_by_height_or_hash: SharedLruCache<String, BlockDetails>,
    connection_pool: ConnectionPool,
    network_status: SharedNetworkStatus,
    contract_address: String,
    mempool_request_sender: mpsc::Sender<MempoolRequest>,
}

impl AppState {
    fn access_storage(&self) -> ActixResult<StorageProcessor> {
        self.connection_pool
            .access_storage_fragile()
            .map_err(|_| HttpResponse::RequestTimeout().finish().into())
    }

    // Spawns future updating SharedNetworkStatus in the current `actix::System`
    fn spawn_network_status_updater(&self, panic_notify: mpsc::Sender<bool>) {
        let state = self.clone();

        std::thread::Builder::new()
            .name("rest-state-updater".to_string())
            .spawn(move || {
                let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());

                let mut runtime = Runtime::new().expect("tokio runtime creation");

                let state_update_task = async move {
                    let mut timer = time::interval(Duration::from_millis(1000));
                    loop {
                        timer.tick().await;

                        let storage = state.connection_pool.access_storage().expect("db failed");

                        let last_verified = storage
                            .chain()
                            .block_schema()
                            .get_last_verified_block()
                            .unwrap_or(0);
                        let status = NetworkStatus {
                            next_block_at_max: None,
                            last_committed: storage
                                .chain()
                                .block_schema()
                                .get_last_committed_block()
                                .unwrap_or(0),
                            last_verified,
                            total_transactions: storage
                                .chain()
                                .stats_schema()
                                .count_total_transactions()
                                .unwrap_or(0),
                            outstanding_txs: storage
                                .chain()
                                .stats_schema()
                                .count_outstanding_proofs(last_verified)
                                .unwrap_or(0),
                        };

                        // save status to state
                        *state.network_status.0.as_ref().write().unwrap() = status;
                    }
                };
                runtime.block_on(state_update_task);
            })
            .expect("State update thread");
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TestnetConfigResponse {
    contract_address: String,
}

fn handle_get_testnet_config(data: web::Data<AppState>) -> ActixResult<HttpResponse> {
    let contract_address = data.contract_address.clone();
    Ok(HttpResponse::Ok().json(TestnetConfigResponse { contract_address }))
}

fn handle_get_network_status(data: web::Data<AppState>) -> ActixResult<HttpResponse> {
    let network_status = data.network_status.read();
    Ok(HttpResponse::Ok().json(network_status))
}

#[derive(Debug, Serialize)]
struct AccountStateResponse {
    // None if account is not created yet.
    id: Option<AccountId>,
    commited: Account,
    verified: Account,
}

fn handle_get_account_state(
    data: web::Data<AppState>,
    account_address: web::Path<Address>,
) -> ActixResult<HttpResponse> {
    let storage = data.access_storage()?;

    let (id, verified, commited) = {
        let stored_account_state = storage
            .chain()
            .account_schema()
            .account_state_by_address(&account_address)
            .map_err(|_| HttpResponse::InternalServerError().finish())?;

        let empty_state = |address: &Address| {
            let mut acc = Account::default();
            acc.address = *address;
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

    let res = AccountStateResponse {
        id,
        commited,
        verified,
    };

    Ok(HttpResponse::Ok().json(res))
}

fn handle_get_tokens(data: web::Data<AppState>) -> ActixResult<HttpResponse> {
    let storage = data.access_storage()?;
    let tokens = storage
        .tokens_schema()
        .load_tokens()
        .map_err(|_| HttpResponse::InternalServerError().finish())?;

    let mut vec_tokens = tokens.values().cloned().collect::<Vec<_>>();
    vec_tokens.sort_by_key(|t| t.id);

    Ok(HttpResponse::Ok().json(vec_tokens))
}

fn handle_get_account_transactions(
    data: web::Data<AppState>,
    address: web::Path<PubKeyHash>,
) -> ActixResult<HttpResponse> {
    let storage = data.access_storage()?;
    let txs = storage
        .chain()
        .operations_ext_schema()
        .get_account_transactions(&address)
        .map_err(|_| HttpResponse::InternalServerError().finish())?;
    Ok(HttpResponse::Ok().json(txs))
}

fn handle_get_account_transactions_history(
    data: web::Data<AppState>,
    request_path: web::Path<(Address, i64, i64)>,
) -> ActixResult<HttpResponse> {
    let (address, offset, limit) = request_path.into_inner();

    const MAX_LIMIT: i64 = 100;
    if limit > MAX_LIMIT {
        return Err(HttpResponse::BadRequest().finish().into());
    }

    let storage = data.access_storage()?;

    let res = storage
        .chain()
        .operations_ext_schema()
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

    let tx_receipt =
        if let Some(tx_receipt) = data.cache_of_transaction_receipts.get(&transaction_hash) {
            Some(tx_receipt)
        } else {
            let storage = data.access_storage()?;
            let tx_receipt = storage
                .chain()
                .operations_ext_schema()
                .tx_receipt(transaction_hash.as_slice())
                .unwrap_or(None);

            if let Some(tx_receipt) = tx_receipt.clone() {
                if tx_receipt.verified {
                    data.cache_of_transaction_receipts
                        .insert(transaction_hash, tx_receipt);
                }
            }

            tx_receipt
        };

    if let Some(tx) = tx_receipt {
        Ok(HttpResponse::Ok().json(tx))
    } else {
        Ok(HttpResponse::Ok().json(()))
    }
}

fn handle_get_tx_by_hash(
    data: web::Data<AppState>,
    hash_hex_with_prefix: web::Path<String>,
) -> ActixResult<HttpResponse> {
    if hash_hex_with_prefix.len() < 2 {
        return Err(HttpResponse::BadRequest().finish().into());
    }

    let hash = {
        let hash = if hash_hex_with_prefix.starts_with("0x") {
            hex::decode(&hash_hex_with_prefix.into_inner()[2..])
        } else if hash_hex_with_prefix.starts_with("sync-tx:") {
            hex::decode(&hash_hex_with_prefix.into_inner()[8..])
        } else {
            return Err(HttpResponse::BadRequest().finish().into());
        };

        hash.map_err(|_| HttpResponse::BadRequest().finish())?
    };

    let storage = data.access_storage()?;

    let res = storage
        .chain()
        .operations_ext_schema()
        .get_tx_by_hash(hash.as_slice())
        .map_err(|_| HttpResponse::InternalServerError().finish())?;

    Ok(HttpResponse::Ok().json(res))
}

fn handle_get_priority_op_receipt(
    data: web::Data<AppState>,
    id: web::Path<u32>,
) -> ActixResult<HttpResponse> {
    let id = id.into_inner();
    let receipt = if let Some(receipt) = data.cache_of_priority_op_receipts.get(&id) {
        receipt
    } else {
        let storage = data.access_storage()?;
        let receipt = storage
            .chain()
            .operations_ext_schema()
            .get_priority_op_receipt(id)
            .map_err(|_| HttpResponse::InternalServerError().finish())?;

        if receipt.verified {
            data.cache_of_priority_op_receipts
                .insert(id, receipt.clone());
        }

        receipt
    };

    Ok(HttpResponse::Ok().json(receipt))
}

fn handle_get_transaction_by_id(
    data: web::Data<AppState>,
    path: web::Path<(u32, u32)>,
) -> ActixResult<HttpResponse> {
    let (block_id, tx_id) = path.into_inner();

    let exec_ops = if let Some(exec_ops) = data.cache_of_block_executed_ops.get(&block_id) {
        exec_ops
    } else {
        let storage = data.access_storage()?;
        let executed_ops = storage
            .chain()
            .block_schema()
            .get_block_executed_ops(block_id)
            .map_err(|_| HttpResponse::InternalServerError().finish())?;

        if let Ok(block_details) = storage.chain().block_schema().load_block_range(block_id, 1) {
            if !block_details.is_empty() && block_details[0].verified_at.is_some() {
                data.cache_of_block_executed_ops
                    .insert(block_id, executed_ops.clone());
            }
        }

        executed_ops
    };

    if let Some(exec_op) = exec_ops.get(tx_id as usize) {
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

    let resp = storage
        .chain()
        .block_schema()
        .load_block_range(max_block, limit)
        .map_err(|e| {
            warn!("handle_get_blocks db fail: {}", e);
            HttpResponse::InternalServerError().finish()
        })?;
    Ok(HttpResponse::Ok().json(resp))
}

fn handle_get_block_by_id(
    data: web::Data<AppState>,
    block_id: web::Path<u32>,
) -> ActixResult<HttpResponse> {
    let block_id = block_id.into_inner();
    let block = if let Some(block) = data.cache_of_blocks_info.get(&block_id) {
        Some(block)
    } else {
        let storage = data.access_storage()?;
        let mut blocks = storage
            .chain()
            .block_schema()
            .load_block_range(block_id, 1)
            .map_err(|_| HttpResponse::InternalServerError().finish())?;

        if !blocks.is_empty() && blocks[0].verified_at.is_some() {
            data.cache_of_blocks_info
                .insert(block_id, blocks[0].clone());
        }

        blocks.pop()
    };
    if let Some(block) = block {
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

    let executed_ops =
        if let Some(executed_ops) = data.cache_of_block_executed_ops.get(&block_id) {
            executed_ops
        } else {
            let storage = data.access_storage()?;
            let executed_ops = storage
                .chain()
                .block_schema()
                .get_block_executed_ops(block_id)
                .map_err(|_| HttpResponse::InternalServerError().finish())?;

            if let Ok(block_details) = storage.chain().block_schema().load_block_range(block_id, 1)
            {
                if !block_details.is_empty() && block_details[0].verified_at.is_some() {
                    data.cache_of_block_executed_ops
                        .insert(block_id, executed_ops.clone());
                }
            }

            executed_ops
        }
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
                ExecutedOperations::Tx(tx) => tx.tx.hash().to_string(),
                ExecutedOperations::PriorityOp(tx) => {
                    format!("0x{}", hex::encode(&tx.priority_op.eth_hash))
                }
            };

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
    let query = query.into_inner().query;
    let block = if let Some(block) = data.cache_blocks_by_height_or_hash.get(&query) {
        Some(block)
    } else {
        let storage = data.access_storage()?;
        let block = storage
            .chain()
            .block_schema()
            .find_block_by_height_or_hash(query.clone());

        if let Some(block) = block.clone() {
            if block.verified_at.is_some() {
                data.cache_blocks_by_height_or_hash.insert(query, block);
            }
        }

        block
    };

    if let Some(block) = block {
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
    mempool_request_sender: mpsc::Sender<MempoolRequest>,
    panic_notify: mpsc::Sender<bool>,
    each_cache_size: usize,
) {
    std::thread::Builder::new()
        .name("actix-rest-api".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());

            let runtime = actix_rt::System::new("api-server");

            let state = AppState {
                cache_of_transaction_receipts: SharedLruCache::new(each_cache_size),
                cache_of_priority_op_receipts: SharedLruCache::new(each_cache_size),
                cache_of_block_executed_ops: SharedLruCache::new(each_cache_size),
                cache_of_blocks_info: SharedLruCache::new(each_cache_size),
                cache_blocks_by_height_or_hash: SharedLruCache::new(each_cache_size),
                connection_pool,
                network_status: SharedNetworkStatus::default(),
                contract_address: format!("{:?}", contract_address),
                mempool_request_sender,
            };
            state.spawn_network_status_updater(panic_notify);

            start_server(state, listen_addr);
            runtime.run().unwrap_or_default();
        })
        .expect("Api server thread");
}
