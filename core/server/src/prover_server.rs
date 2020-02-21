mod pool;

// Built-in
use std::sync::{Arc, RwLock};
use std::thread;
use std::{net, time};
// External
use actix_web::{web, App, HttpResponse, HttpServer};
use futures::channel::mpsc;
use log::{error, info, trace};
// Workspace deps
use models::config_options::ThreadPanicNotify;
use prover::client;

struct AppState {
    connection_pool: storage::ConnectionPool,
    preparing_data_pool: Arc<RwLock<pool::ProversDataPool>>,
    prover_timeout: time::Duration,
}

impl AppState {
    fn access_storage(&self) -> actix_web::Result<storage::StorageProcessor> {
        self.connection_pool
            .access_storage()
            .map_err(actix_web::error::ErrorInternalServerError)
    }
}

fn register(
    data: web::Data<AppState>,
    r: web::Json<client::ProverReq>,
) -> actix_web::Result<String> {
    info!("register request for prover with name: {}", r.name);
    if r.name == "" {
        return Err(actix_web::error::ErrorBadRequest("empty name"));
    }
    let storage = data.access_storage()?;
    let id = storage
        .register_prover(&r.name)
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(id.to_string())
}

fn block_to_prove(
    data: web::Data<AppState>,
    r: web::Json<client::ProverReq>,
) -> actix_web::Result<HttpResponse> {
    trace!("request block to prove from worker: {}", r.name);
    if r.name == "" {
        return Err(actix_web::error::ErrorBadRequest("empty name"));
    }
    let storage = data.access_storage()?;
    let ret = storage
        .prover_run_for_next_commit(&r.name, data.prover_timeout)
        .map_err(|e| {
            error!("could not get next unverified commit operation: {}", e);
            actix_web::error::ErrorInternalServerError("storage layer error")
        })?;
    if let Some(prover_run) = ret {
        Ok(HttpResponse::Ok().json(client::BlockToProveRes {
            prover_run_id: prover_run.id,
            block: prover_run.block_number,
        }))
    } else {
        Ok(HttpResponse::Ok().json(client::BlockToProveRes {
            prover_run_id: 0,
            block: 0,
        }))
    }
}

fn prover_data(
    data: web::Data<AppState>,
    block: web::Json<i64>,
) -> actix_web::Result<HttpResponse> {
    info!("requesting prover_data for block {}", *block);
    let data_pool = data
        .preparing_data_pool
        .read()
        .expect("failed to get read lock on data");
    Ok(HttpResponse::Ok().json(data_pool.get(*block)))
}

fn working_on(
    data: web::Data<AppState>,
    r: web::Json<client::WorkingOnReq>,
) -> actix_web::Result<()> {
    trace!(
        "working on request for prover_run with id: {}",
        r.prover_run_id
    );
    let storage = data
        .connection_pool
        .access_storage()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    storage
        .record_prover_is_working(r.prover_run_id)
        .map_err(|e| {
            error!("failed to record prover work in progress request: {}", e);
            actix_web::error::ErrorInternalServerError("storage layer error")
        })
}

fn publish(data: web::Data<AppState>, r: web::Json<client::PublishReq>) -> actix_web::Result<()> {
    info!("publish of a proof for block: {}", r.block);
    let storage = data
        .connection_pool
        .access_storage()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    match storage.store_proof(r.block, &r.proof) {
        Ok(_) => {
            let mut data_pool = data
                .preparing_data_pool
                .write()
                .expect("failed to get write lock on data");
            data_pool.clean_up(r.block as i64);
            Ok(())
        }
        Err(e) => {
            error!("failed to store received proof: {}", e);
            Err(actix_web::error::ErrorInternalServerError(
                "storage layer error",
            ))
        }
    }
}

fn stopped(data: web::Data<AppState>, prover_id: web::Json<i32>) -> actix_web::Result<()> {
    info!(
        "prover sent stopped request with prover_run id: {}",
        prover_id
    );
    let storage = data
        .connection_pool
        .access_storage()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    storage.record_prover_stop(*prover_id).map_err(|e| {
        error!("failed to record prover stop: {}", e);
        actix_web::error::ErrorInternalServerError("storage layer error")
    })
}

pub fn start_prover_server(
    connection_pool: storage::ConnectionPool,
    bind_to: net::SocketAddr,
    prover_timeout: time::Duration,
    rounds_interval: time::Duration,
    panic_notify: mpsc::Sender<bool>,
) {
    let panic_notify2 = panic_notify.clone();
    thread::Builder::new()
        .name("prover_server".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify);
            let data_pool = Arc::new(RwLock::new(pool::ProversDataPool::new()));

            // Start pool maintainer thread.
            let pool_maintainer = pool::ProverPoolMaintainer::new(
                connection_pool.clone(),
                Arc::clone(&data_pool),
                rounds_interval,
            );
            pool_maintainer.start_maintain_routine(panic_notify2);

            // Start HTTP server.
            HttpServer::new(move || {
                App::new()
                    .wrap(actix_web::middleware::Logger::default())
                    .data(AppState {
                        connection_pool: connection_pool.clone(),
                        preparing_data_pool: Arc::clone(&data_pool),
                        prover_timeout,
                    })
                    .route("/register", web::post().to(register))
                    .route("/block_to_prove", web::get().to(block_to_prove))
                    .route("/working_on", web::post().to(working_on))
                    .route("/prover_data", web::get().to(prover_data))
                    .route("/publish", web::post().to(publish))
                    .route("/stopped", web::post().to(stopped))
            })
            .bind(&bind_to)
            .expect("failed to bind")
            .run()
            .expect("failed to run server");
        })
        .expect("failed to start prover server");
}
