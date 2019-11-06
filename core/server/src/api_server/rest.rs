use actix_cors::Cors;
use actix_web::{
    middleware,
    web::{self},
    App, HttpResponse, HttpServer, Result as ActixResult,
};
use models::node::ExecutedOperations;
use models::NetworkStatus;
use std::sync::mpsc;
use storage::{ConnectionPool, StorageProcessor};

use crate::ThreadPanicNotify;
use futures::{Future, Stream};
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::timer::Interval;

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
                    .route("/blocks/{block_id}", web::get().to(handle_get_block_by_id))
                    .route("/blocks", web::get().to(handle_get_blocks))
                    .route("/search", web::get().to(handle_block_search)),
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
            };
            state.spawn_network_status_updater();

            start_server(state, listen_addr);
            runtime.run().unwrap_or_default();
        })
        .expect("Api server thread");
}
