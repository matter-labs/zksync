// Built-in
use std::sync::Arc;
use std::thread;
use std::time::Duration;
// External
use actix_web::dev::ServiceRequest;
use actix_web::{web, App, HttpResponse, HttpServer};
use actix_web_httpauth::extractors::{
    bearer::{BearerAuth, Config},
    AuthenticationError,
};
use actix_web_httpauth::middleware::HttpAuthentication;
use futures::channel::mpsc;
use jsonwebtoken::errors::Error as JwtError;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
// Workspace deps
use zksync_config::ProverOptions;
use zksync_storage::{ConnectionPool, StorageProcessor};
// Local deps
use self::scaler::ScalerOracle;
use zksync_circuit::serialization::ProverData;
use zksync_prover_utils::api::{
    JobRequestData, JobResultData, ProverInputRequest, ProverInputResponse, ProverOutputRequest,
    WorkingOn,
};
use zksync_types::aggregated_operations::{
    AggregatedActionType, AggregatedOperation, BlocksCreateProofOperation,
};
use zksync_types::prover::{
    ProverJobType, AGGREGATED_PROOF_JOB_PRIORITY, SINGLE_PROOF_JOB_PRIORITY,
};
use zksync_utils::panic_notify::ThreadPanicNotify;

mod scaler;
mod witness_generator;

#[derive(Debug, Serialize, Deserialize)]
struct PayloadAuthToken {
    /// Subject (whom auth token refers to).
    sub: String,
    /// Expiration time (as UTC timestamp).
    exp: usize,
}

#[derive(Debug, Clone)]
struct AppState {
    secret_auth: String,
    connection_pool: zksync_storage::ConnectionPool,
    scaler_oracle: Arc<RwLock<ScalerOracle>>,
    prover_timeout: Duration,
}

impl AppState {
    pub fn new(
        secret_auth: String,
        connection_pool: ConnectionPool,
        prover_timeout: Duration,
        idle_provers: u32,
    ) -> Self {
        let scaler_oracle = Arc::new(RwLock::new(ScalerOracle::new(
            connection_pool.clone(),
            idle_provers,
        )));

        Self {
            secret_auth,
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

/// The structure that stores the secret key for checking JsonWebToken matching.
struct AuthTokenValidator<'a> {
    decoding_key: DecodingKey<'a>,
}

impl<'a> AuthTokenValidator<'a> {
    fn new(secret: &'a str) -> Self {
        Self {
            decoding_key: DecodingKey::from_secret(secret.as_ref()),
        }
    }

    /// Checks whether the secret key and the authorization token match.
    fn validate_auth_token(&self, token: &str) -> Result<(), JwtError> {
        decode::<PayloadAuthToken>(token, &self.decoding_key, &Validation::default())?;

        Ok(())
    }

    async fn validator(
        &self,
        req: ServiceRequest,
        credentials: BearerAuth,
    ) -> actix_web::Result<ServiceRequest> {
        let config = req.app_data::<Config>().cloned().unwrap_or_default();

        self.validate_auth_token(credentials.token())
            .map_err(|_| AuthenticationError::from(config))?;

        Ok(req)
    }
}

async fn status() -> actix_web::Result<String> {
    Ok("alive".into())
}

async fn get_job(
    data: web::Data<AppState>,
    r: web::Json<ProverInputRequest>,
) -> actix_web::Result<HttpResponse> {
    log::trace!("request block to prove from worker: {}", r.prover_name);
    if r.prover_name == "" {
        return Err(actix_web::error::ErrorBadRequest("empty name"));
    }
    let mut storage = data.access_storage().await?;
    let ret = storage
        .prover_schema()
        .get_idle_prover_job_from_job_queue()
        .await
        .map_err(|e| {
            vlog::warn!("could not get next unverified commit operation: {}", e);
            actix_web::error::ErrorInternalServerError("storage layer error")
        })?;
    if let Some(prover_job) = ret {
        log::info!("satisfied request to prove from worker");
        Ok(HttpResponse::Ok().json(ProverInputResponse {
            job_id: prover_job.job_id,
            first_block: prover_job.first_block,
            last_block: prover_job.last_block,
            data: Some(
                serde_json::from_value(prover_job.job_data)
                    .expect("Failed to parse prover job from db"),
            ),
        }))
    } else {
        Ok(HttpResponse::Ok().json(ProverInputResponse {
            job_id: 0,
            first_block: 0,
            last_block: 0,
            data: None,
        }))
    }
}

async fn working_on(
    data: web::Data<AppState>,
    r: web::Json<WorkingOn>,
) -> actix_web::Result<HttpResponse> {
    // These heartbeats aren't really important, as they're sent
    // continuously while prover is performing computations.
    log::trace!("Received heartbeat for prover_run with id: {}", r.job_id);
    let mut storage = data
        .access_storage()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    storage
        .prover_schema()
        .record_prover_is_working(r.job_id, &r.prover_name)
        .await
        .map_err(|e| {
            vlog::warn!("failed to record prover work in progress request: {}", e);
            actix_web::error::ErrorInternalServerError("storage layer error")
        })?;

    Ok(HttpResponse::Ok().finish())
}

async fn publish(
    data: web::Data<AppState>,
    r: web::Json<ProverOutputRequest>,
) -> actix_web::Result<HttpResponse> {
    let mut storage = data
        .access_storage()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let storage_result = match &r.data {
        JobResultData::BlockProof(single_proof) => {
            log::info!(
                "Received a proof for job: {}, single block: {}",
                r.job_id,
                r.first_block
            );
            storage
                .prover_schema()
                .store_proof(r.first_block, single_proof)
                .await
        }
        JobResultData::AggregatedBlockProof(aggregated_proof) => {
            log::info!(
                "Received a proof for job: {}, aggregated blocks: [{},{}]",
                r.job_id,
                r.first_block,
                r.last_block
            );
            storage
                .prover_schema()
                .store_aggregated_proof(r.job_id, r.first_block, r.last_block, aggregated_proof)
                .await
        }
    };
    if let Err(e) = storage_result {
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
    prover_name: web::Json<String>,
) -> actix_web::Result<HttpResponse> {
    let mut storage = data
        .access_storage()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    log::info!(
        "Prover instance '{}' send a stopping notification",
        &prover_name
    );

    storage
        .prover_schema()
        .record_prover_stop(&prover_name)
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
    let mut oracle = data.scaler_oracle.write().await;

    let needed_count = oracle
        .provers_required()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let response = RequiredReplicasOutput { needed_count };

    Ok(HttpResponse::Ok().json(response))
}

async fn update_prover_job_queue_loop(connection_pool: ConnectionPool) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;

        if let Ok(mut storage) = connection_pool.access_storage().await {
            update_prover_job_queue(&mut storage)
                .await
                .unwrap_or_default();
        }
    }
}

async fn update_prover_job_queue(storage: &mut StorageProcessor<'_>) -> anyhow::Result<()> {
    {
        let mut prover_schema = storage.prover_schema();
        let next_single_block_to_add = prover_schema
            .get_last_block_prover_job_queue(ProverJobType::SingleProof)
            .await?
            + 1;
        let witness_for_next_single_block =
            prover_schema.get_witness(next_single_block_to_add).await?;
        if let Some(witness) = witness_for_next_single_block {
            let prover_data: ProverData =
                serde_json::from_value(witness).expect("incorrect single block witness");
            let block_size = prover_data.operations.len();
            let job_data =
                serde_json::to_value(JobRequestData::BlockProof(prover_data, block_size))
                    .expect("Failed to serialize single proof job data");
            prover_schema
                .add_prover_job_to_job_queue(
                    next_single_block_to_add,
                    next_single_block_to_add,
                    job_data,
                    SINGLE_PROOF_JOB_PRIORITY,
                    ProverJobType::SingleProof,
                )
                .await?;
        }
    }

    {
        let next_aggregated_proof_block = storage
            .prover_schema()
            .get_last_block_prover_job_queue(ProverJobType::AggregatedProof)
            .await?
            + 1;
        let create_block_proof_action = storage
            .chain()
            .operations_schema()
            .get_aggregated_op_that_affects_block(
                AggregatedActionType::CreateProofBlocks,
                next_aggregated_proof_block,
            )
            .await?;
        if let Some((
            _,
            AggregatedOperation::CreateProofBlocks(BlocksCreateProofOperation { blocks, .. }),
        )) = create_block_proof_action
        {
            let first_block = blocks
                .first()
                .map(|b| b.block_number)
                .expect("should have 1 block");
            let last_block = blocks
                .last()
                .map(|b| b.block_number)
                .expect("should have 1 block");
            let mut data = Vec::new();
            for block in blocks {
                let proof = storage
                    .prover_schema()
                    .load_proof(block.block_number)
                    .await?
                    .expect("Single proof should exist");
                let block_size = block.block_chunks_size;
                data.push((proof, block_size));
            }
            let job_data = serde_json::to_value(JobRequestData::AggregatedBlockProof(data))
                .expect("Failed to serialize aggregated proof job");
            storage
                .prover_schema()
                .add_prover_job_to_job_queue(
                    first_block,
                    last_block,
                    job_data,
                    AGGREGATED_PROOF_JOB_PRIORITY,
                    ProverJobType::AggregatedProof,
                )
                .await?;
        }
    }
    storage.prover_schema().mark_stale_jobs_as_idle().await?;
    Ok(())
}

pub fn run_prover_server(
    connection_pool: zksync_storage::ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
    prover_options: ProverOptions,
) {
    thread::Builder::new()
        .name("prover_server".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());
            let mut actix_runtime = actix_rt::System::new("prover-server");

            actix_runtime.block_on(async move {
                tokio::spawn(update_prover_job_queue_loop(connection_pool.clone()));

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
                for offset in 0..prover_options.witness_generators {
                    let start_block = (last_verified_block + offset + 1) as u32;
                    let block_step = prover_options.witness_generators as u32;
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
                let secret_auth = prover_options.secret_auth.clone();
                let gone_timeout = prover_options.gone_timeout;
                let idle_provers = prover_options.idle_provers;
                HttpServer::new(move || {
                    let app_state = AppState::new(
                        secret_auth.clone(),
                        connection_pool.clone(),
                        gone_timeout,
                        idle_provers,
                    );

                    let auth = HttpAuthentication::bearer(move |req, credentials| async {
                        let secret_auth = req
                            .app_data::<web::Data<AppState>>()
                            .expect("failed get AppState upon receipt of the authentication token")
                            .secret_auth
                            .clone();
                        AuthTokenValidator::new(&secret_auth)
                            .validator(req, credentials)
                            .await
                    });

                    // By calling `register_data` instead of `data` we're avoiding double
                    // `Arc` wrapping of the object.
                    App::new()
                        .wrap(auth)
                        .app_data(web::Data::new(app_state))
                        .route("/status", web::get().to(status))
                        .route("/get_job", web::get().to(get_job))
                        .route("/working_on", web::post().to(working_on))
                        .route("/publish", web::post().to(publish))
                        .route("/stopped", web::post().to(stopped))
                        .route(
                            "/api/internal/prover/replicas",
                            web::post().to(required_replicas),
                        )
                })
                .bind(&prover_options.prover_server_address)
                .expect("failed to bind")
                .run()
                .await
            })
        })
        .expect("failed to start prover server");
}
