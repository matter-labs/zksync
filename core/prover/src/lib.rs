pub mod client;
pub mod exit_proof;
pub mod prover_data;
pub mod serialization;

// Built-in deps
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::{fmt, thread, time};
// External deps
use crate::franklin_crypto::bellman::pairing::ff::PrimeField;
use log::*;
// Workspace deps
use crypto_exports::franklin_crypto;
use crypto_exports::franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use crypto_exports::franklin_crypto::rescue::bn256::Bn256RescueParams;
use models::prover_utils::EncodedProofPlonk;
use models::prover_utils::{PlonkVerificationKey, SetupForStepByStepProver};

/// We prepare some data before making proof for each block size, so we cache it in case next block
/// would be of our size
struct PreparedComputations {
    block_size: usize,
    setup: SetupForStepByStepProver,
}

pub struct BabyProver<C: ApiClient> {
    block_sizes: Vec<usize>,
    prepared_computations: Mutex<Option<PreparedComputations>>,
    api_client: C,
    heartbeat_interval: time::Duration,
    stop_signal: Arc<AtomicBool>,
}

pub trait ApiClient {
    fn block_to_prove(&self, block_size: usize) -> Result<Option<(i64, i32)>, failure::Error>;
    fn working_on(&self, job_id: i32) -> Result<(), failure::Error>;
    fn prover_data(&self, block: i64) -> Result<prover_data::ProverData, failure::Error>;
    fn publish(&self, block: i64, p: EncodedProofPlonk) -> Result<(), failure::Error>;
}

#[derive(Debug)]
pub enum BabyProverError {
    Api(String),
    Internal(String),
    Stop,
}

impl fmt::Display for BabyProverError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let desc = match self {
            BabyProverError::Api(s) => s,
            BabyProverError::Internal(s) => s,
            BabyProverError::Stop => "stop",
        };
        write!(f, "{}", desc)
    }
}

pub fn start<C: 'static + Sync + Send + ApiClient>(
    prover: BabyProver<C>,
    exit_err_tx: mpsc::Sender<BabyProverError>,
) {
    let (tx_block_start, rx_block_start) = mpsc::channel();
    let prover = Arc::new(prover);
    let prover_rc = Arc::clone(&prover);
    let join_handle = thread::spawn(move || {
        let tx_block_start2 = tx_block_start.clone();
        exit_err_tx
            .send(prover.run_rounds(tx_block_start))
            .expect("failed to send exit error");
        tx_block_start2
            .send((0, true))
            .expect("failed to send heartbeat exit request"); // exit heartbeat routine request.
    });
    prover_rc.keep_sending_work_heartbeats(rx_block_start);
    join_handle
        .join()
        .expect("failed to join on running rounds thread");
}

impl<C: ApiClient> BabyProver<C> {
    pub fn new(
        block_sizes: Vec<usize>,
        api_client: C,
        heartbeat_interval: time::Duration,
        stop_signal: Arc<AtomicBool>,
    ) -> Self {
        assert!(!block_sizes.is_empty());
        BabyProver {
            block_sizes,
            prepared_computations: Mutex::new(None),
            api_client,
            heartbeat_interval,
            stop_signal,
        }
    }

    fn run_rounds(&self, start_heartbeats_tx: mpsc::Sender<(i32, bool)>) -> BabyProverError {
        let pause_duration = time::Duration::from_secs(models::node::config::PROVER_CYCLE_WAIT);

        info!("Running worker rounds");

        while !self.stop_signal.load(Ordering::SeqCst) {
            trace!("Starting a next round");
            let ret = self.next_round(&start_heartbeats_tx);
            if let Err(err) = ret {
                match err {
                    BabyProverError::Api(text) => {
                        error!("could not reach api server: {}", text);
                    }
                    BabyProverError::Internal(_) => {
                        return err;
                    }
                    _ => {}
                };
            }
            trace!("round completed.");
            thread::sleep(pause_duration);
        }
        BabyProverError::Stop
    }

    fn next_round(
        &self,
        start_heartbeats_tx: &mpsc::Sender<(i32, bool)>,
    ) -> Result<(), BabyProverError> {
        let block_size_idx_to_try_first =
            if let Some(precomp) = self.prepared_computations.lock().unwrap().as_ref() {
                self.block_sizes
                    .iter()
                    .position(|size| *size == precomp.block_size)
                    .unwrap()
            } else {
                0
            };

        let (mut block, mut job_id, mut block_size) = (0, 0, 0);
        for offset_idx in 0..self.block_sizes.len() {
            let idx = (block_size_idx_to_try_first + offset_idx) % self.block_sizes.len();
            let current_block_size = self.block_sizes[idx];

            let block_to_prove =
                self.api_client
                    .block_to_prove(current_block_size)
                    .map_err(|e| {
                        let e = format!("failed to get block to prove {}", e);
                        BabyProverError::Api(e)
                    })?;

            let (current_request_block, current_request_job_id) =
                block_to_prove.unwrap_or_else(|| {
                    trace!(
                        "no block to prove from the server for size: {}",
                        current_block_size
                    );
                    (0, 0)
                });

            if current_request_job_id != 0 {
                block = current_request_block;
                job_id = current_request_job_id;
                block_size = current_block_size;
                break;
            }
        }

        // Notify heartbeat routine on new proving block job or None.
        start_heartbeats_tx
            .send((job_id, false))
            .expect("failed to send new job to heartbeat routine");
        if job_id == 0 {
            return Ok(());
        }
        let prover_data = self.api_client.prover_data(block).map_err(|err| {
            BabyProverError::Api(format!(
                "could not get prover data for block {}: {}",
                block, err
            ))
        })?;

        info!(
            "starting to compute proof for block {}, size: {}",
            block, block_size
        );

        let instance = circuit::circuit::FranklinCircuit {
            rescue_params: &models::params::RESCUE_PARAMS as &Bn256RescueParams,
            jubjub_params: &models::params::JUBJUB_PARAMS as &AltJubjubBn256,
            operation_batch_size: block_size,
            old_root: Some(prover_data.old_root),
            new_root: Some(prover_data.new_root),
            block_number: models::node::Fr::from_str(&block.to_string()),
            validator_address: Some(prover_data.validator_address),
            pub_data_commitment: Some(prover_data.public_data_commitment),
            operations: prover_data.operations,
            validator_balances: prover_data.validator_balances,
            validator_audit_path: prover_data.validator_audit_path,
            validator_account: prover_data.validator_account,
        };

        // we do this way here so old precomp is dropped
        let valid_cached_precomp = {
            self.prepared_computations
                .lock()
                .unwrap()
                .take()
                .filter(|p| p.block_size == block_size)
        };
        let precomp = if let Some(precomp) = valid_cached_precomp {
            precomp
        } else {
            let setup =
                SetupForStepByStepProver::prepare_setup_for_step_by_step_prover(instance.clone())
                    .map_err(|e| {
                    BabyProverError::Internal(format!(
                        "Failed to prepare setup for block_size: {}, err: {}",
                        block_size, e
                    ))
                })?;
            PreparedComputations { block_size, setup }
        };

        let vk = PlonkVerificationKey::read_verification_key_for_main_circuit(block_size).map_err(
            |e| {
                BabyProverError::Internal(format!(
                    "Failed to read vk for block: {}, size: {}, err: {}",
                    block, block_size, e
                ))
            },
        )?;
        let verified_proof = precomp
            .setup
            .gen_step_by_step_proof_using_prepared_setup(instance, &vk)
            .map_err(|e| {
                BabyProverError::Internal(format!(
                    "Failed to create verified proof for block: {}, size: {}, err: {}",
                    block, block_size, e
                ))
            })?;

        *self.prepared_computations.lock().unwrap() = Some(precomp);

        self.api_client
            .publish(block, verified_proof)
            .map_err(|e| BabyProverError::Api(format!("failed to publish proof: {}", e)))?;

        info!("finished and published proof for block {}", block);
        Ok(())
    }

    fn keep_sending_work_heartbeats(&self, start_heartbeats_rx: mpsc::Receiver<(i32, bool)>) {
        let mut job_id = 0;
        loop {
            thread::sleep(self.heartbeat_interval);
            let (j, quit) = match start_heartbeats_rx.try_recv() {
                Ok(v) => v,
                Err(mpsc::TryRecvError::Empty) => (job_id, false),
                Err(e) => {
                    panic!("error receiving from hearbeat channel: {}", e);
                }
            };
            if quit {
                return;
            }
            job_id = j;
            if job_id != 0 {
                trace!("sending working_on request for job_id: {}", job_id);
                let ret = self.api_client.working_on(job_id);
                if let Err(e) = ret {
                    error!("working_on request errored: {}", e);
                }
            }
        }
    }
}
