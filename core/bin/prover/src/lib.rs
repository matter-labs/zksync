pub mod cli_utils;
pub mod client;
pub mod exit_proof;
pub mod plonk_step_by_step_prover;

// Built-in deps
use futures::{pin_mut, FutureExt};
use std::sync::{
    atomic::{AtomicBool, AtomicI32, Ordering},
    Arc,
};
use std::time::Duration;
use std::{
    fmt::{self, Debug},
    thread,
};
use tokio::sync::{mpsc, oneshot};
// External deps
use zksync_crypto::rand::{
    distributions::{IndependentSample, Range},
    thread_rng,
};
// Workspace deps
use tokio::stream::StreamExt;
use zksync_config::ProverOptions;
use zksync_crypto::proof::EncodedAggregatedProof;
use zksync_crypto::{proof::EncodedProofPlonk, Engine};
use zksync_prover_utils::aggregated_proofs::{AggregatedProof, SingleProof};
use zksync_prover_utils::api::{
    JobRequestData, JobResultData, ProverId, ProverInputRequest, ProverInputRequestAuxData,
    ProverInputResponse, ProverOutputRequest,
};
use zksync_utils::panic_notify::ThreadPanicNotify;

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
    /// Config concrete type used by current prover
    type Config: ProverConfig;
    /// Creates prover from config and API client.
    fn create_from_config(config: Self::Config) -> Self;
    fn get_request_aux_data(&self) -> ProverInputRequestAuxData {
        Default::default()
    }
    /// Resource heavy operation
    fn create_proof(&self, data: JobRequestData) -> Result<JobResultData, anyhow::Error>;
}
#[async_trait::async_trait]
pub trait ApiClient: Debug {
    async fn get_job(&self, req: ProverInputRequest) -> Result<ProverInputResponse, anyhow::Error>;
    async fn working_on(&self, job_id: i32) -> Result<(), anyhow::Error>;
    async fn publish(&self, data: ProverOutputRequest) -> Result<(), anyhow::Error>;
    async fn prover_stopped(&self, prover_id: ProverId) -> Result<(), anyhow::Error>;
}

async fn compute_proof_no_blocking<PROVER>(
    prover: PROVER,
    data: JobRequestData,
) -> anyhow::Result<(PROVER, JobResultData)>
where
    PROVER: ProverImpl + Send + Sync + 'static,
{
    let (mut result_receiver, mut panic_receiver) = {
        let (result_sender, result_receiver) = oneshot::channel();
        let (panic_sender, panic_receiver) = oneshot::channel();
        std::thread::spawn(move || {
            // TODO: panic sender should work
            // std::panic::set_hook(Box::new(|panic_info| {
            //     log::error!("Prover panicked: {}", panic_info);
            //     panic_sender.send(Err(anyhow::format_err!("Prover panicked")));
            // }));

            let prover_with_proof = prover.create_proof(data).map(|proof| (prover, proof));
            result_sender.send(prover_with_proof);
        });
        (result_receiver.fuse(), panic_receiver.fuse())
    };

    futures::select! {
        res = result_receiver => res?,
        pan = panic_receiver => pan?,
    }
}

async fn prover_work_cycle<PROVER, CLIENT>(
    mut prover: PROVER,
    client: CLIENT,
    shutdown: ShutdownRequest,
    prover_options: ProverOptions,
) where
    CLIENT: 'static + Sync + Send + ApiClient,
    PROVER: ProverImpl + Send + Sync + 'static,
{
    let prover_name = String::from("localhost");

    let mut new_job_poll_timer = tokio::time::interval(prover_options.cycle_wait);
    loop {
        new_job_poll_timer.tick().await;

        if shutdown.get() {
            break;
        }

        let aux_data = prover.get_request_aux_data();
        let prover_input_response = match client
            .get_job(ProverInputRequest {
                prover_name: prover_name.clone(),
                aux_data,
            })
            .await
        {
            Ok(job) => job,
            Err(e) => {
                log::warn!("Failed to get job for prover: {}", e);
                continue;
            }
        };

        let ProverInputResponse {
            job_id,
            data: job_data,
        } = prover_input_response;
        let job_data = if let Some(job_data) = job_data {
            job_data
        } else {
            continue;
        };

        let mut heartbeat_future_handle = async {
            loop {
                let timeout_value = {
                    let between = Range::new(0.8f64, 2.0);
                    let mut rng = thread_rng();
                    let random_multiplier = between.ind_sample(&mut rng);
                    Duration::from_secs(
                        (prover_options.heartbeat_interval.as_secs_f64() * random_multiplier)
                            as u64,
                    )
                };

                tokio::time::delay_for(timeout_value).await;
                client
                    .working_on(job_id)
                    .await
                    .map_err(|e| log::warn!("Failed to send hearbeat"))
                    .unwrap_or_default();
            }
        }
        .fuse();
        pin_mut!(heartbeat_future_handle);

        let mut compute_proof_future = compute_proof_no_blocking(prover, job_data).fuse();
        pin_mut!(compute_proof_future);

        let (ret_prover, proof) = futures::select! {
            comp_proof = compute_proof_future => {
                comp_proof.expect("Failed to compute proof")
            },
            x = heartbeat_future_handle => { unreachable!() },
        };
        prover = ret_prover;

        client
            .publish(ProverOutputRequest {
                job_id,
                data: proof,
            })
            .await
            .map_err(|e| log::warn!("Failed to publish proof: {}", e))
            .unwrap_or_default();
    }
}
