pub mod cli_utils;
pub mod client;
pub mod exit_proof;
pub mod plonk_step_by_step_prover;

// Built-in deps
use std::sync::{
    atomic::{AtomicBool, AtomicI32, Ordering},
    mpsc, Arc,
};
use std::time::Duration;
use std::{
    fmt::{self, Debug},
    thread,
};
// External deps
use zksync_crypto::rand::Rng;
// Workspace deps
use zksync_config::ProverOptions;
use zksync_crypto::{proof::EncodedProofPlonk, Engine};

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
pub trait ProverImpl<C: ApiClient> {
    /// Config concrete type used by current prover
    type Config: ProverConfig;
    /// Creates prover from config and API client.
    fn create_from_config(config: Self::Config, client: C, heartbeat: Duration) -> Self;
    /// Fetches job from the server and creates proof for it
    fn next_round(
        &self,
        start_heartbeats_tx: mpsc::Sender<(i32, bool)>,
    ) -> Result<(), BabyProverError>;
    /// Returns client reference and config needed for heartbeat.
    fn get_heartbeat_options(&self) -> (&C, Duration);
}

pub trait ApiClient: Debug {
    fn block_to_prove(&self, block_size: usize) -> Result<Option<(i64, i32)>, anyhow::Error>;
    fn working_on(&self, job_id: i32) -> Result<(), anyhow::Error>;
    fn prover_data(
        &self,
        block: i64,
    ) -> Result<zksync_circuit::circuit::ZkSyncCircuit<'_, Engine>, anyhow::Error>;
    fn publish(&self, block: i64, p: EncodedProofPlonk) -> Result<(), anyhow::Error>;
    fn prover_stopped(&self, prover_run_id: i32) -> Result<(), anyhow::Error>;
}

#[derive(Debug)]
pub enum BabyProverError {
    Api(String),
    Internal(String),
}

impl fmt::Display for BabyProverError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let desc = match self {
            BabyProverError::Api(s) => s,
            BabyProverError::Internal(s) => s,
        };
        write!(f, "{}", desc)
    }
}

pub fn start<CLIENT, PROVER>(
    prover: PROVER,
    exit_err_tx: mpsc::Sender<BabyProverError>,
    shutdown_requested: ShutdownRequest,
) where
    CLIENT: 'static + Sync + Send + ApiClient,
    PROVER: ProverImpl<CLIENT> + Send + Sync + 'static,
{
    let (tx_block_start, rx_block_start) = mpsc::channel();
    let prover = Arc::new(prover);
    let prover_rc = Arc::clone(&prover);
    let join_handle = thread::spawn(move || {
        let tx_block_start2 = tx_block_start.clone();
        exit_err_tx
            .send(run_rounds(
                prover.as_ref(),
                tx_block_start,
                shutdown_requested,
            ))
            .expect("failed to send exit error");
        tx_block_start2
            .send((0, true))
            .expect("failed to send heartbeat exit request"); // exit heartbeat routine request.
    });
    let (client, heartbeat_interval) = prover_rc.get_heartbeat_options();
    keep_sending_work_heartbeats(client, heartbeat_interval, rx_block_start);
    join_handle
        .join()
        .expect("failed to join on running rounds thread");
}

fn run_rounds<PROVER: ProverImpl<CLIENT>, CLIENT: ApiClient>(
    prover: &PROVER,
    start_heartbeats_tx: mpsc::Sender<(i32, bool)>,
    shutdown_request: ShutdownRequest,
) -> BabyProverError {
    log::info!("Running worker rounds");
    let cycle_wait_interval = ProverOptions::from_env().cycle_wait;

    loop {
        if shutdown_request.get() {
            log::info!("Shutdown requested, ignoring the next round and finishing the job");

            let prover_id = shutdown_request.prover_id();
            if prover_id != ABSENT_PROVER_ID {
                let (api_client, _) = prover.get_heartbeat_options();
                match api_client.prover_stopped(prover_id) {
                    Ok(_) => {}
                    Err(e) => log::error!("failed to send prover stop request: {}", e),
                }
            }

            std::process::exit(0);
        }

        log::trace!("Starting a next round");
        let ret = prover.next_round(start_heartbeats_tx.clone());
        if let Err(err) = ret {
            match err {
                BabyProverError::Api(text) => {
                    log::error!("could not reach api server: {}", text);
                }
                BabyProverError::Internal(_) => {
                    return err;
                }
            };
        }
        log::trace!("round completed.");

        // Randomly generated shift to desynchronize multiple provers started at the same time.
        let mut rng = zksync_crypto::rand::thread_rng();
        let sleep_shift_ms = rng.gen_range(0, 300);
        let sleep_duration = cycle_wait_interval + Duration::from_millis(sleep_shift_ms);
        thread::sleep(sleep_duration);
    }
}

fn keep_sending_work_heartbeats<C: ApiClient>(
    client: &C,
    heartbeat_interval: Duration,
    start_heartbeats_rx: mpsc::Receiver<(i32, bool)>,
) {
    let mut job_id = 0;
    loop {
        let mut rng = zksync_crypto::rand::thread_rng();

        // Randomly generated shift, so multiple provers won't spam the server at the same time.
        let sleep_shift_ms = rng.gen_range(0, 500);
        let sleep_duration = heartbeat_interval + Duration::from_millis(sleep_shift_ms);
        thread::sleep(sleep_duration);

        // Loop is required to empty queue: prover may send multiple messages while heartbeat
        // thread was asleep, and we must process only the last one.
        // This loop exists as soon as message queue is empty.
        loop {
            match start_heartbeats_rx.try_recv() {
                Ok((new_job_id, quit_now)) => {
                    // Check if we should stop this thread immediately.
                    if quit_now {
                        return;
                    }
                    // Update the current job ID.
                    if new_job_id != 0 {
                        // Message with non-zero job ID is sent once per job, so it won't be spammed all over the log.
                        log::info!(
                            "Starting sending heartbeats for job with ID: {}",
                            new_job_id
                        );
                    }
                    job_id = new_job_id;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No messages in queue, use the last received value.
                    break;
                }
                Err(e) => {
                    panic!("error receiving from heartbeat channel: {}", e);
                }
            };
        }
        if job_id != 0 {
            log::trace!("sending working_on request for job_id: {}", job_id);
            let ret = client.working_on(job_id);
            if let Err(e) = ret {
                log::error!("working_on request erred: {}", e);
            }
        }
    }
}
