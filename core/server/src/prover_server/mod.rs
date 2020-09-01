// Built-in
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{self, Duration};
// External
use actix_web::{web, App, HttpResponse, HttpServer};
use futures::channel::mpsc;
use log::{info, trace};
// Workspace deps
use models::config_options::ConfigurationOptions;
use models::{config_options::ThreadPanicNotify, node::BlockNumber};
use prover::client;
use storage::ConnectionPool;
// Local deps
use crate::prover_server::scaler::ScalerOracle;

mod scaler;
mod witness_generator;

#[derive(Debug)]
struct AppState {
    connection_pool: storage::ConnectionPool,
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

fn prover_data(
    data: web::Data<AppState>,
    block: web::Json<BlockNumber>,
) -> actix_web::Result<HttpResponse> {
    trace!("Got request for prover_data for block {}", *block);
    let storage = data
        .access_storage()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let witness = match storage.prover_schema().get_witness(block.0) {
        Ok(witness) => witness,
        Err(_) => return Ok(HttpResponse::InternalServerError().finish()),
    };
    if witness.is_some() {
        info!("Sent prover_data for block {}", *block);
    } else {
        // No witness, we should just wait
        warn!("No witness for block {}", *block);
    }
    Ok(HttpResponse::Ok().json(witness))
}

fn working_on(
    data: web::Data<AppState>,
    r: web::Json<client::WorkingOnReq>,
) -> actix_web::Result<()> {
    // These heartbeats aren't really important, as they're sent
    // continuously while prover is performing computations.
    trace!(
        "Received heartbeat for prover_run with id: {}",
        r.prover_run_id
    );
    let storage = data
        .access_storage()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    storage
        .prover_schema()
        .record_prover_is_working(r.prover_run_id)
        .map_err(|e| {
            vlog::warn!("failed to record prover work in progress request: {}", e);
            actix_web::error::ErrorInternalServerError("storage layer error")
        })
}

fn publish(data: web::Data<AppState>, r: web::Json<client::PublishReq>) -> actix_web::Result<()> {
    info!("Received a proof for block: {}", r.block);
    let storage = data
        .access_storage()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    match storage.prover_schema().store_proof(r.block, &r.proof) {
        Ok(_) => Ok(()),
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
    prover_timeout: time::Duration,
    rounds_interval: time::Duration,
    panic_notify: mpsc::Sender<bool>,
    config_options: ConfigurationOptions,
) {
    thread::Builder::new()
        .name("prover_server".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());

            // Start pool maintainer threads.
            for offset in 0..config_options.witness_generators {
                let start_block = 1 + offset as u32;
                let block_step = config_options.witness_generators as u32;
                info!(
                    "Starting witness generator ({},{})",
                    start_block, block_step
                );
                let pool_maintainer = witness_generator::WitnessGenerator::new(
                    connection_pool.clone(),
                    rounds_interval,
                    start_block,
                    block_step,
                );
                pool_maintainer.start(panic_notify.clone());
            }

            // Start HTTP server.
            let idle_provers = config_options.idle_provers;
            HttpServer::new(move || {
                let app_state =
                    AppState::new(connection_pool.clone(), prover_timeout, idle_provers);

                // By calling `register_data` instead of `data` we're avoiding double
                // `Arc` wrapping of the object.
                App::new()
                    .wrap(actix_web::middleware::Logger::default())
                    .register_data(web::Data::new(app_state))
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
            .expect("failed to run server");
        })
        .expect("failed to start prover server");
}
