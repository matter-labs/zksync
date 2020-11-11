// Built-in
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
// External
use actix_web::{web, App, HttpResponse, HttpServer};
use futures::channel::mpsc;
use serde::{Deserialize, Serialize};
// Workspace deps
use zksync_config::{ConfigurationOptions, ProverOptions};
use zksync_prover_utils::api::{BlockToProveRes, ProverReq, PublishReq, WorkingOnReq};
use zksync_storage::ConnectionPool;
use zksync_types::BlockNumber;
// Local deps
use self::scaler::ScalerOracle;
use zksync_utils::panic_notify::ThreadPanicNotify;

mod scaler;
mod witness_generator;

#[derive(Debug)]
struct AppState {
    connection_pool: zksync_storage::ConnectionPool,
    scaler_oracle: Arc<RwLock<ScalerOracle>>,
    prover_timeout: Duration,
}

impl AppState {
    pub fn new(
        connection_pool: ConnectionPool,
        prover_timeout: Duration,
        idle_provers: u32,
    ) -> Self {
        let scaler_oracle = Arc::new(RwLock::new(ScalerOracle::new(
            connection_pool.clone(),
            idle_provers,
        )));

        Self {
            connection_pool,
            scaler_oracle,
            prover_timeout,
        }
    }

    async fn access_storage(&self) -> actix_web::Result<zksync_storage::StorageProcessor<'_>> {
        self.connection_pool.access_storage().await.map_err(|e| {
            vlog::warn!("Failed to access storage: {}", e);
            actix_web::error::ErrorInternalServerError(e)
        })
    }
}

async fn status() -> actix_web::Result<String> {
    Ok("alive".into())
}

async fn register(data: web::Data<AppState>, r: web::Json<ProverReq>) -> actix_web::Result<String> {
    log::info!("register request for prover with name: {}", r.name);
    if r.name == "" {
        return Err(actix_web::error::ErrorBadRequest("empty name"));
    }
    let mut storage = data.access_storage().await?;
    let id = storage
        .prover_schema()
        .register_prover(&r.name, r.block_size)
        .await
        .map_err(|e| {
            vlog::warn!("Failed to register prover in the db: {}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;
    Ok(id.to_string())
}

async fn block_to_prove(
    data: web::Data<AppState>,
    r: web::Json<ProverReq>,
) -> actix_web::Result<HttpResponse> {
    log::trace!("request block to prove from worker: {}", r.name);
    if r.name == "" {
        return Err(actix_web::error::ErrorBadRequest("empty name"));
    }
    let mut storage = data.access_storage().await?;
    let ret = storage
        .prover_schema()
        .prover_run_for_next_commit(&r.name, data.prover_timeout, r.block_size)
        .await
        .map_err(|e| {
            vlog::warn!("could not get next unverified commit operation: {}", e);
            actix_web::error::ErrorInternalServerError("storage layer error")
        })?;
    if let Some(prover_run) = ret {
        log::info!(
            "satisfied request block {} to prove from worker: {}",
            prover_run.block_number,
            r.name
        );
        Ok(HttpResponse::Ok().json(BlockToProveRes {
            prover_run_id: prover_run.id,
            block: prover_run.block_number,
        }))
    } else {
        Ok(HttpResponse::Ok().json(BlockToProveRes {
            prover_run_id: 0,
            block: 0,
        }))
    }
}

async fn prover_data(
    data: web::Data<AppState>,
    block: web::Json<BlockNumber>,
) -> actix_web::Result<HttpResponse> {
    log::trace!("Got request for prover_data for block {}", *block);
    let mut storage = data
        .access_storage()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let witness = match storage.prover_schema().get_witness(block.0).await {
        Ok(witness) => witness,
        Err(_) => return Ok(HttpResponse::InternalServerError().finish()),
    };
    if witness.is_some() {
        log::info!("Sent prover_data for block {}", *block);
    } else {
        // No witness, we should just wait
        log::warn!("No witness for block {}", *block);
    }
    Ok(HttpResponse::Ok().json(witness))
}

async fn working_on(
    data: web::Data<AppState>,
    r: web::Json<WorkingOnReq>,
) -> actix_web::Result<HttpResponse> {
    // These heartbeats aren't really important, as they're sent
    // continuously while prover is performing computations.
    log::trace!(
        "Received heartbeat for prover_run with id: {}",
        r.prover_run_id
    );
    let mut storage = data
        .access_storage()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    storage
        .prover_schema()
        .record_prover_is_working(r.prover_run_id)
        .await
        .map_err(|e| {
            vlog::warn!("failed to record prover work in progress request: {}", e);
            actix_web::error::ErrorInternalServerError("storage layer error")
        })?;

    Ok(HttpResponse::Ok().finish())
}

async fn publish(
    data: web::Data<AppState>,
    r: web::Json<PublishReq>,
) -> actix_web::Result<HttpResponse> {
    log::info!("Received a proof for block: {}", r.block);
    let mut storage = data
        .access_storage()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    if let Err(e) = storage.prover_schema().store_proof(r.block, &r.proof).await {
        vlog::error!("failed to store received proof: {}", e);
        let message = if e.to_string().contains("duplicate key") {
            "duplicate key"
        } else {
            "storage layer error"
        };
        return Err(actix_web::error::ErrorInternalServerError(message));
    }

    Ok(HttpResponse::Ok().finish())
}

async fn stopped(
    data: web::Data<AppState>,
    prover_id: web::Json<i32>,
) -> actix_web::Result<HttpResponse> {
    let prover_id = prover_id.into_inner();

    let mut storage = data
        .access_storage()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let prover_description = storage
        .prover_schema()
        .prover_by_id(prover_id)
        .await
        .map_err(|_| {
            vlog::warn!(
                "Received stop notification from an unknown prover with ID {}",
                prover_id
            );
            actix_web::error::ErrorBadRequest("unknown prover ID")
        })?;

    log::info!(
        "Prover instance '{}' with ID {} send a stopping notification",
        prover_description.worker,
        prover_id
    );

    storage
        .prover_schema()
        .record_prover_stop(prover_id)
        .await
        .map_err(|e| {
            vlog::warn!("failed to record prover stop: {}", e);
            actix_web::error::ErrorInternalServerError("storage layer error")
        })?;

    Ok(HttpResponse::Ok().finish())
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

async fn required_replicas(
    data: web::Data<AppState>,
    _input: web::Json<RequiredReplicasInput>,
) -> actix_web::Result<HttpResponse> {
    let mut oracle = data.scaler_oracle.write().expect("Expected write lock");

    let needed_count = oracle
        .provers_required()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let response = RequiredReplicasOutput { needed_count };

    Ok(HttpResponse::Ok().json(response))
}

pub fn run_prover_server(
    connection_pool: zksync_storage::ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
    prover_options: ProverOptions,
    config_options: ConfigurationOptions,
) {
    thread::Builder::new()
        .name("prover_server".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());
            let mut actix_runtime = actix_rt::System::new("prover-server");

            actix_runtime.block_on(async move {
                let last_verified_block = {
                    let mut storage = connection_pool
                        .access_storage()
                        .await
                        .expect("Failed to access storage");

                    storage
                        .chain()
                        .block_schema()
                        .get_last_verified_block()
                        .await
                        .expect("Failed to get last verified block number")
                        as usize
                };

                // Start pool maintainer threads.
                for offset in 0..config_options.witness_generators {
                    let start_block = (last_verified_block + offset + 1) as u32;
                    let block_step = config_options.witness_generators as u32;
                    log::info!(
                        "Starting witness generator ({},{})",
                        start_block,
                        block_step
                    );
                    let pool_maintainer = witness_generator::WitnessGenerator::new(
                        connection_pool.clone(),
                        prover_options.prepare_data_interval,
                        start_block,
                        block_step,
                    );
                    pool_maintainer.start(panic_notify.clone());
                }

                // Start HTTP server.
                let idle_provers = config_options.idle_provers;
                HttpServer::new(move || {
                    let app_state = AppState::new(
                        connection_pool.clone(),
                        prover_options.gone_timeout,
                        idle_provers,
                    );

                    // By calling `register_data` instead of `data` we're avoiding double
                    // `Arc` wrapping of the object.
                    App::new()
                        .wrap(actix_web::middleware::Logger::default())
                        .app_data(web::Data::new(app_state))
                        .route("/status", web::get().to(status))
                        .route("/register", web::post().to(register))
                        .route("/block_to_prove", web::get().to(block_to_prove))
                        .route("/working_on", web::post().to(working_on))
                        .route("/prover_data", web::get().to(prover_data))
                        .route("/publish", web::post().to(publish))
                        .route("/stopped", web::post().to(stopped))
                        .route(
                            "/api/internal/prover/replicas",
                            web::post().to(required_replicas),
                        )
                })
                .bind(&config_options.prover_server_address)
                .expect("failed to bind")
                .run()
                .await
            })
        })
        .expect("failed to start prover server");
}
