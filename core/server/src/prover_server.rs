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
use models::node::BlockNumber;
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

fn status() -> actix_web::Result<String> {
    Ok("alive".into())
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
        .register_prover(&r.name, r.block_size)
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
        .prover_run_for_next_commit(&r.name, data.prover_timeout, r.block_size)
        .map_err(|e| {
            error!("could not get next unverified commit operation: {}", e);
            actix_web::error::ErrorInternalServerError("storage layer error")
        })?;
    if let Some(prover_run) = ret {
        info!(
            "satisfied request block {} to prove from worker: {}",
            prover_run.block_number, r.name
        );
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
    block: web::Json<BlockNumber>,
) -> actix_web::Result<HttpResponse> {
    trace!("requesting prover_data for block {}", *block);
    let data_pool = data
        .preparing_data_pool
        .read()
        .expect("failed to get read lock on data");
    let res = data_pool.get(*block);
    if res.is_some() {
        info!("Sent prover_data for block {}", *block);
    }
    Ok(HttpResponse::Ok().json(res))
}

fn working_on(
    data: web::Data<AppState>,
    r: web::Json<client::WorkingOnReq>,
) -> actix_web::Result<()> {
    info!(
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
            data_pool.clean_up(r.block);
            Ok(())
        }
        Err(e) => {
            error!("failed to store received proof: {}", e);
            let message = if e.to_string().contains("duplicate key") {
                "duplicate key"
            } else {
                "storage layer error"
            };
            Err(actix_web::error::ErrorInternalServerError(message))
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
    thread::Builder::new()
        .name("prover_server".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());
            let data_pool = Arc::new(RwLock::new(pool::ProversDataPool::new(10)));

            // Start pool maintainer thread.
            let pool_maintainer = pool::Maintainer::new(
                connection_pool.clone(),
                Arc::clone(&data_pool),
                rounds_interval,
            );
            pool_maintainer.start(panic_notify);

            // Start HTTP server.
            HttpServer::new(move || {
                App::new()
                    .wrap(actix_web::middleware::Logger::default())
                    .data(AppState {
                        connection_pool: connection_pool.clone(),
                        preparing_data_pool: data_pool.clone(),
                        prover_timeout,
                    })
                    .route("/status", web::get().to(status))
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
