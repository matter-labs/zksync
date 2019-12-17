pub mod witness_generator;

// Built-in deps
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::{thread, time};
// External deps
use bellman::groth16;
use ff::PrimeField;
use log::error;
use pairing::bn256;
// Workspace deps

pub struct Worker<C: ApiClient> {
    circuit_params: groth16::Parameters<bn256::Bn256>,
    jubjub_params: franklin_crypto::alt_babyjubjub::AltJubjubBn256,
    api_client: C,
    heartbeat_interval: time::Duration,
    stop_signal: Arc<AtomicBool>,
}

pub trait ApiClient {
    fn block_to_prove(&self) -> Result<Option<(i64, i32)>, String>;
    fn working_on(&self, job_id: i32);
    fn prover_data(
        &self,
        block: i64,
        timeout: time::Duration,
    ) -> Result<witness_generator::ProverData, String>;
    fn publish(
        &self,
        block: i64,
        p: groth16::Proof<models::node::Engine>,
        public_data_commitment: models::node::Fr,
    ) -> Result<(), String>;
}

pub fn start<'a, C: 'static + Sync + Send + ApiClient>(prover: Worker<C>) {
    let (tx_block_start, rx_block_start) = mpsc::channel();
    let prover = Arc::new(prover);
    let prover_rc = Arc::clone(&prover);
    let join_handle = thread::spawn(move || {
        prover.run_rounds(tx_block_start);
    });
    prover_rc.keep_sending_work_heartbeats(rx_block_start);
    join_handle.join();
}

impl<C: ApiClient> Worker<C> {
    pub fn new(
        circuit_params: groth16::Parameters<bn256::Bn256>,
        jubjub_params: franklin_crypto::alt_babyjubjub::AltJubjubBn256,
        api_client: C,
        heartbeat_interval: time::Duration,
        stop_signal: Arc<AtomicBool>,
    ) -> Self {
        Worker {
            circuit_params,
            jubjub_params,
            api_client,
            heartbeat_interval,
            stop_signal,
        }
    }

    fn run_rounds(&self, start_heartbeats_tx: mpsc::Sender<i32>) {
        // TODO: add PROVER_CYCLE_WAIT usage
        let mut rng = rand::OsRng::new().unwrap();

        while !self.stop_signal.load(Ordering::SeqCst) {
            let block_to_prove = self.api_client.block_to_prove();
            let block_to_prove = match block_to_prove {
                Ok(b) => b,
                // TODO: log error
                _ => continue,
            };

            let (block, job_id) = match block_to_prove {
                Some(b) => b,
                _ => (0, 0),
            };
            // Notify heartbeat routine on new proving block or None.
            start_heartbeats_tx.send(job_id);
            if job_id == 0 {
                continue;
            }
            // TODO: timeout
            let prover_data = match self
                .api_client
                .prover_data(block, time::Duration::from_secs(10))
            {
                Ok(v) => v,
                Err(err) => {
                    error!("could not get prover data for block {}: {}", block, err);
                    continue;
                }
            };

            let instance = circuit::circuit::FranklinCircuit {
                params: &self.jubjub_params,
                operation_batch_size: models::params::block_size_chunks(),
                old_root: Some(prover_data.old_root),
                new_root: Some(prover_data.new_root),
                block_number: models::node::Fr::from_str(&(block).to_string()),
                validator_address: Some(prover_data.validator_address),
                pub_data_commitment: Some(prover_data.public_data_commitment),
                operations: prover_data.operations,
                validator_balances: prover_data.validator_balances,
                validator_audit_path: prover_data.validator_audit_path,
                validator_account: prover_data.validator_account,
            };

            let proof =
                bellman::groth16::create_random_proof(instance, &self.circuit_params, &mut rng);

            if proof.is_err() {
                // TODO: panic?
                panic!("proof can not be created: {}", proof.err().unwrap());
            }

            // TODO: handle error.
            let p = proof.unwrap();

            let pvk = bellman::groth16::prepare_verifying_key(&self.circuit_params.vk);

            let res = bellman::groth16::verify_proof(
                &pvk,
                &p.clone(),
                &[prover_data.public_data_commitment],
            );
            if res.is_err() {
                panic!("err")
                // return Err("Proof verification has failed".to_owned());
            }
            if !res.unwrap() {
                panic!("err")
                // return Err("Proof is invalid".to_owned());
            }

            self.api_client
                .publish(block, p, prover_data.public_data_commitment);
        }
    }

    fn keep_sending_work_heartbeats(&self, start_heartbeats_rx: mpsc::Receiver<i32>) {
        let mut job_id = 0;
        while !self.stop_signal.load(Ordering::SeqCst) {
            thread::sleep(self.heartbeat_interval);
            job_id = match start_heartbeats_rx.try_recv() {
                Ok(v) => v,
                Err(mpsc::TryRecvError::Empty) => job_id,
                Err(e) => break,
                _ => 0,
            };
            if job_id != 0 {
                self.api_client.working_on(job_id);
            }
        }
    }
}
