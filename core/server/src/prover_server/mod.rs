// Built-in
use std::sync::{Arc, RwLock};
use std::thread;
use std::{
    net,
    time::{self, Duration},
};
// External
use actix_web::{web, App, HttpResponse, HttpServer};
use futures::channel::mpsc;
use log::{info, trace};
// Workspace deps
use models::{circuit::CircuitAccountTree, config_options::ThreadPanicNotify, node::BlockNumber};
use prover::{client, ProverJob};
use storage::ConnectionPool;
// Local deps
use crate::prover_server::scaler::ScalerOracle;
use models::prover_utils::EncodedProofPlonk;
use prover::client::MultiblockDataReq;

mod pool;
mod scaler;

#[derive(Debug)]
struct AppState {
    connection_pool: storage::ConnectionPool,
    preparing_data_pool: Arc<RwLock<pool::ProversDataPool>>,
    scaler_oracle: Arc<RwLock<ScalerOracle>>,
    prover_timeout: Duration,
    blocks_batch_timeout: Duration,
    max_block_batch_size: usize,
}

impl AppState {
    pub fn new(
        connection_pool: ConnectionPool,
        preparing_data_pool: Arc<RwLock<pool::ProversDataPool>>,
        prover_timeout: Duration,
        blocks_batch_timeout: Duration,
        max_block_batch_size: usize,
        idle_provers: u32,
    ) -> Self {
        let scaler_oracle = Arc::new(RwLock::new(ScalerOracle::new(
            connection_pool.clone(),
            idle_provers,
        )));

        Self {
            connection_pool,
            preparing_data_pool,
            scaler_oracle,
            prover_timeout,
            blocks_batch_timeout,
            max_block_batch_size,
        }
    }

    fn access_storage(&self) -> actix_web::Result<storage::StorageProcessor> {
        self.connection_pool.access_storage_fragile().map_err(|e| {
            vlog::warn!("Failed to access storage: {}", e);
            actix_web::error::ErrorInternalServerError(e)
        })
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
        .prover_schema()
        .register_prover(&r.name, r.block_size)
        .map_err(|e| {
            vlog::warn!("Failed to register prover in the db: {}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;
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
        .prover_schema()
        .prover_run_for_next_commit(&r.name, data.prover_timeout, r.block_size)
        .map_err(|e| {
            vlog::warn!("could not get next unverified commit operation: {}", e);
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

fn multiblock_to_prove(
    data: web::Data<AppState>,
    r: web::Json<client::ProverMultiblockReq>,
) -> actix_web::Result<HttpResponse> {
    trace!("request multiblock to prove from worker: {}", r.name);
    if r.name == "" {
        return Err(actix_web::error::ErrorBadRequest("empty name"));
    }
    let storage = data.access_storage()?;
    let ret = storage
        .prover_schema()
        .prover_multiblock_run(
            &r.name,
            data.prover_timeout,
            data.blocks_batch_timeout,
            data.max_block_batch_size,
        )
        .map_err(|e| {
            vlog::warn!("could not get next unverified block sequence: {}", e);
            actix_web::error::ErrorInternalServerError("storage layer error")
        })?;
    if let Some(prover_run) = ret {
        info!(
            "satisfied request multiblock with indexes [{};{}] to prove from worker: {}",
            prover_run.block_number_from, prover_run.block_number_to, r.name
        );
        Ok(HttpResponse::Ok().json(client::MultiblockToProveRes {
            prover_run_id: prover_run.id,
            block_from: prover_run.block_number_from,
            block_to: prover_run.block_number_to,
        }))
    } else {
        Ok(HttpResponse::Ok().json(client::MultiblockToProveRes {
            prover_run_id: 0,
            block_from: 0,
            block_to: 0,
        }))
    }
}

fn prover_block_data(
    data: web::Data<AppState>,
    block: web::Json<BlockNumber>,
) -> actix_web::Result<HttpResponse> {
    trace!("Got request for prover_block_data for block {}", *block);
    let data_pool = data
        .preparing_data_pool
        .read()
        .expect("failed to get read lock on data");
    let res = data_pool.get(*block);
    if res.is_some() {
        info!("Sent prover_block_data for block {}", *block);
    }
    Ok(HttpResponse::Ok().json(res))
}

fn prover_multiblock_data(
    data: web::Data<AppState>,
    r: web::Json<MultiblockDataReq>,
) -> actix_web::Result<HttpResponse> {
    trace!(
        "Got request for prover_multiblock_data for multiblock [{};{}]",
        r.block_from,
        r.block_to
    );
    let storage = data
        .access_storage()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let mut res: Vec<(EncodedProofPlonk, usize)> = vec![];
    for block_num in r.block_from..=r.block_to {
        let current_block_proof = storage
            .prover_schema()
            .load_proof(block_num as BlockNumber)
            .map_err(|e| {
                vlog::warn!("failed to load proof of block {}: {}", block_num, e);
                actix_web::error::ErrorInternalServerError("storage layer error")
            });
        let current_block = storage
            .chain()
            .block_schema()
            .get_block(block_num as BlockNumber)
            .map_err(|e| {
                vlog::warn!("failed to load block {}: {}", block_num, e);
                actix_web::error::ErrorInternalServerError("storage layer error")
            })?;
        match current_block_proof {
            Ok(proof) => {
                res.push((
                    proof,
                    current_block
                        .expect("block must be loaded")
                        .block_chunks_size,
                ));
            }
            Err(e) => {
                warn!(
                    "prover requested prover_multiblock_data for multiblock [{};{}], but proof for block {} is not already stored in db: {}",
                    r.block_from,
                    r.block_to,
                    block_num,
                    e
                );

                return Ok(HttpResponse::Ok().json(Option::<Vec<(EncodedProofPlonk, usize)>>::None));
            }
        }
    }
    info!(
        "Sent prover_multiblock_data for multiblock [{};{}]",
        r.block_from, r.block_to
    );
    Ok(HttpResponse::Ok().json(Some(res)))
}

fn working_on(
    data: web::Data<AppState>,
    r: web::Json<client::WorkingOnReq>,
) -> actix_web::Result<()> {
    // These heartbeats aren't really important, as they're sent
    // continuously while prover is performing computations.
    trace!("Received heartbeat for prover_run: {:?}", r.prover_run);
    let storage = data
        .access_storage()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    match r.prover_run {
        ProverJob::BlockProve(job_id) => storage
            .prover_schema()
            .record_prover_is_working(job_id)
            .map_err(|e| {
                vlog::warn!("failed to record prover work in progress request: {}", e);
                actix_web::error::ErrorInternalServerError("storage layer error")
            }),
        ProverJob::MultiblockProve(job_id) => storage
            .prover_schema()
            .record_prover_multiblock_is_working(job_id)
            .map_err(|e| {
                vlog::warn!(
                    "failed to record prover multiblock work in progress request: {}",
                    e
                );
                actix_web::error::ErrorInternalServerError("storage layer error")
            }),
    }
}

fn publish_block(
    data: web::Data<AppState>,
    r: web::Json<client::PublishReq>,
) -> actix_web::Result<()> {
    info!("Received a proof for block: {}", r.block);
    let storage = data
        .access_storage()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    match storage.prover_schema().store_proof(r.block, &r.proof) {
        Ok(_) => {
            let mut data_pool = data
                .preparing_data_pool
                .write()
                .expect("failed to get write lock on data");
            data_pool.clean_up(r.block);
            Ok(())
        }
        Err(e) => {
            vlog::error!("failed to store received proof: {}", e);
            let message = if e.to_string().contains("duplicate key") {
                "duplicate key"
            } else {
                "storage layer error"
            };
            Err(actix_web::error::ErrorInternalServerError(message))
        }
    }
}

fn publish_multiblock(
    data: web::Data<AppState>,
    r: web::Json<client::PublishMultiblockReq>,
) -> actix_web::Result<()> {
    info!(
        "Received a proof for multiblock: [{};{}]",
        r.block_from, r.block_to
    );
    let storage = data
        .access_storage()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    match storage
        .prover_schema()
        .store_multiblock_proof(r.block_from, r.block_to, &r.proof)
    {
        Ok(_) => Ok(()),
        Err(e) => {
            vlog::error!("failed to store received multiblock proof: {}", e);
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
    let prover_id = prover_id.into_inner();

    let storage = data
        .access_storage()
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let prover_description = storage
        .prover_schema()
        .prover_by_id(prover_id)
        .map_err(|_| {
            vlog::warn!(
                "Received stop notification from an unknown prover with ID {}",
                prover_id
            );
            actix_web::error::ErrorBadRequest("unknown prover ID")
        })?;

    info!(
        "Prover instance '{}' with ID {} send a stopping notification",
        prover_description.worker, prover_id
    );

    storage
        .prover_schema()
        .record_prover_stop(prover_id)
        .map_err(|e| {
            vlog::warn!("failed to record prover stop: {}", e);
            actix_web::error::ErrorInternalServerError("storage layer error")
        })
}

/// Input of the `/scaler/replicas` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredReplicasInput {
    /// Amount of currently running prover entities.
    current_count: u32,
}

/// Output of the `/scaler/replicas` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredReplicasOutput {
    /// Amount of the prover entities required for server
    /// to run optimally.
    needed_count: u32,
}

fn required_replicas(
    data: web::Data<AppState>,
    _input: web::Json<RequiredReplicasInput>,
) -> actix_web::Result<HttpResponse> {
    let mut oracle = data.scaler_oracle.write().expect("Expected write lock");

    let needed_count = oracle
        .provers_required()
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let response = RequiredReplicasOutput { needed_count };

    Ok(HttpResponse::Ok().json(response))
}

#[allow(clippy::too_many_arguments)]
pub fn start_prover_server(
    connection_pool: storage::ConnectionPool,
    bind_to: net::SocketAddr,
    prover_timeout: time::Duration,
    blocks_batch_timeout: time::Duration,
    max_block_batch_size: usize,
    rounds_interval: time::Duration,
    panic_notify: mpsc::Sender<bool>,
    account_tree: CircuitAccountTree,
    tree_block_number: BlockNumber,
    idle_provers: u32,
) {
    thread::Builder::new()
        .name("prover_server".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());
            let data_pool = Arc::new(RwLock::new(pool::ProversDataPool::new(
                tree_block_number,
                10,
            )));

            // Start pool maintainer thread.
            let pool_maintainer = pool::Maintainer::new(
                connection_pool.clone(),
                Arc::clone(&data_pool),
                rounds_interval,
                account_tree,
                tree_block_number,
            );
            pool_maintainer.start(panic_notify);

            // Start HTTP server.
            HttpServer::new(move || {
                let app_state = AppState::new(
                    connection_pool.clone(),
                    data_pool.clone(),
                    prover_timeout,
                    blocks_batch_timeout,
                    max_block_batch_size,
                    idle_provers,
                );

                // By calling `register_data` instead of `data` we're avoiding double
                // `Arc` wrapping of the object.
                App::new()
                    .wrap(actix_web::middleware::Logger::default())
                    .register_data(web::Data::new(app_state))
                    .route("/status", web::get().to(status))
                    .route("/register", web::post().to(register))
                    .route("/block_to_prove", web::get().to(block_to_prove))
                    .route("/multiblock_to_prove", web::get().to(multiblock_to_prove))
                    .route("/working_on", web::post().to(working_on))
                    .route("/prover_block_data", web::get().to(prover_block_data))
                    .route(
                        "/prover_multiblock_data",
                        web::get().to(prover_multiblock_data),
                    )
                    .route("/publish_block", web::post().to(publish_block))
                    .route("/publish_multiblock", web::post().to(publish_multiblock))
                    .route("/stopped", web::post().to(stopped))
                    .route(
                        "/api/internal/prover/replicas",
                        web::post().to(required_replicas),
                    )
            })
            .bind(&bind_to)
            .expect("failed to bind")
            .run()
            .expect("failed to run server");
        })
        .expect("failed to start prover server");
}
