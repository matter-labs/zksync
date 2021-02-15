pub mod auth_utils;
pub mod cli_utils;
pub mod client;
pub mod dummy_prover;
pub mod plonk_step_by_step_prover;

// Built-in deps
use futures::{pin_mut, FutureExt};
use std::fmt::Debug;
use std::sync::{
    atomic::{AtomicBool, AtomicI32, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::sync::oneshot;
// External deps
use zksync_crypto::rand::{
    distributions::{IndependentSample, Range},
    thread_rng,
};
// Workspace deps
use zksync_config::ProverConfig as EnvProverConfig;
use zksync_prover_utils::api::{
    JobRequestData, JobResultData, ProverInputRequest, ProverInputRequestAuxData,
    ProverInputResponse, ProverOutputRequest,
};

const ABSENT_PROVER_ID: i32 = -1;

#[derive(Debug, Clone)]
pub struct ShutdownRequest {
    shutdown_requested: Arc<AtomicBool>,
    prover_id: Arc<AtomicI32>,
}

impl Default for ShutdownRequest {
    fn default() -> Self {
        let prover_id = Arc::new(AtomicI32::from(ABSENT_PROVER_ID));

        Self {
            shutdown_requested: Default::default(),
            prover_id,
        }
    }
}

impl ShutdownRequest {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_prover_id(&self, id: i32) {
        self.prover_id.store(id, Ordering::SeqCst);
    }

    pub fn prover_id(&self) -> i32 {
        self.prover_id.load(Ordering::SeqCst)
    }

    pub fn set(&self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
    }

    pub fn get(&self) -> bool {
        self.shutdown_requested.load(Ordering::SeqCst)
    }
}

/// Trait that provides type needed by prover to initialize.
pub trait ProverConfig {
    fn from_env() -> Self;
}

/// Trait that tries to separate prover from networking (API)
/// It is still assumed that prover will use ApiClient methods to fetch data from server, but it
/// allows to use common code for all provers (like sending heartbeats, registering prover, etc.)
pub trait ProverImpl {
    /// Config concrete type used by current prover.
    type Config: ProverConfig;
    /// Creates prover from config and API client.
    fn create_from_config(config: Self::Config) -> Self;
    fn get_request_aux_data(&self) -> ProverInputRequestAuxData {
        Default::default()
        // TODO: Add the ability to define different config (ZKS-283).
    }
    /// Resource heavy operation
    fn create_proof(&self, data: JobRequestData) -> anyhow::Result<JobResultData>;
}
#[async_trait::async_trait]
pub trait ApiClient: Debug {
    async fn get_job(&self, req: ProverInputRequest) -> anyhow::Result<ProverInputResponse>;
    async fn working_on(&self, job_id: i32, prover_name: &str) -> anyhow::Result<()>;
    async fn publish(&self, data: ProverOutputRequest) -> anyhow::Result<()>;
    async fn prover_stopped(&self, prover_name: String) -> anyhow::Result<()>;
}

async fn compute_proof_no_blocking<PROVER>(
    prover: PROVER,
    data: JobRequestData,
) -> anyhow::Result<(PROVER, JobResultData)>
where
    PROVER: ProverImpl + Send + Sync + 'static,
{
    let (result_sender, result_receiver) = oneshot::channel();
    std::thread::spawn(move || {
        let prover_with_proof = prover.create_proof(data).map(|proof| (prover, proof));
        result_sender.send(prover_with_proof).unwrap_or_default();
    });
    result_receiver.await?
}

/// Endlessly sends requests to the server, in case of not receiving a response
/// notifies about it in the logs, but does not quit.
async fn heartbeat_future_handle<CLIENT>(
    client: CLIENT,
    prover_name: &str,
    job_id: i32,
    heartbeat_interval: Duration,
) where
    CLIENT: 'static + Sync + Send + ApiClient,
{
    loop {
        let timeout_value = {
            let between = Range::new(0.8f64, 2.0);
            let mut rng = thread_rng();
            let random_multiplier = between.ind_sample(&mut rng);
            Duration::from_secs((heartbeat_interval.as_secs_f64() * random_multiplier) as u64)
        };
        tokio::time::delay_for(timeout_value).await;

        vlog::info!("Starting sending heartbeats for job with ID: {}", job_id);

        client
            .working_on(job_id, &prover_name)
            .await
            .map_err(|e| vlog::warn!("Failed to send heartbeat: {}", e))
            .unwrap_or_default();
    }
}

pub async fn prover_work_cycle<PROVER, CLIENT>(
    mut prover: PROVER,
    client: CLIENT,
    shutdown: ShutdownRequest,
    prover_options: EnvProverConfig,
    prover_name: &str,
) where
    CLIENT: 'static + Sync + Send + ApiClient + Clone,
    PROVER: ProverImpl + Send + Sync + 'static,
{
    vlog::info!("Running worker cycle");
    let mut new_job_poll_timer = tokio::time::interval(prover_options.prover.cycle_wait());
    loop {
        new_job_poll_timer.tick().await;

        if shutdown.get() {
            break;
        }

        let aux_data = prover.get_request_aux_data();
        let prover_input_response = match client
            .get_job(ProverInputRequest {
                prover_name: prover_name.to_string(),
                aux_data,
            })
            .await
        {
            Ok(job) => job,
            Err(e) => {
                vlog::warn!("Failed to get job for prover: {}", e);
                continue;
            }
        };

        let ProverInputResponse {
            job_id,
            data: job_data,
            first_block,
            last_block,
        } = prover_input_response;
        let job_data = if let Some(job_data) = job_data {
            job_data
        } else {
            continue;
        };

        vlog::info!(
            "got job id: {}, blocks: [{}, {}]",
            job_id,
            first_block,
            last_block
        );

        let heartbeat_future_handle = heartbeat_future_handle(
            client.clone(),
            prover_name,
            job_id,
            prover_options.prover.heartbeat_interval(),
        )
        .fuse();
        let compute_proof_future = compute_proof_no_blocking(prover, job_data).fuse();

        pin_mut!(heartbeat_future_handle, compute_proof_future);

        vlog::info!(
            "starting to compute proof for blocks: [{}, {}]",
            first_block,
            last_block
        );

        let (ret_prover, proof) = futures::select! {
            comp_proof = compute_proof_future => {
                comp_proof.expect("Failed to compute proof")
            },
            _ = heartbeat_future_handle => unreachable!(),
        };
        prover = ret_prover;

        client
            .publish(ProverOutputRequest {
                job_id,
                first_block,
                last_block,
                data: proof,
            })
            .await
            .map_err(|e| vlog::warn!("Failed to publish proof: {}", e))
            .unwrap_or_default();

        vlog::info!(
            "finished and published proof for blocks: [{}, {}]",
            first_block,
            last_block
        );
    }
}
