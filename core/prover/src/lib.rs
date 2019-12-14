pub mod witness_generator;

// Built-in uses
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::{thread, time};
// External uses
use bellman::groth16;
use ff::PrimeField;
use log::error;
use pairing::bn256;
// Workspace uses
use witness_generator::ProverData;

pub struct Prover<C: ApiClient> {
    circuit_params: groth16::Parameters<bn256::Bn256>,
    jubjub_params: franklin_crypto::alt_babyjubjub::AltJubjubBn256,
    api_client: C,
    heartbeat_interval: time::Duration,
    stop_signal: Arc<AtomicBool>,
}

pub trait ApiClient {
    // TODO: fn block_to_prove(&self) -> Result<Option<(i64, ProverData)>, String>
    fn block_to_prove(&self) -> Result<Option<i64>, String>;
    fn working_on(&self, block: i64);
    fn prover_data(&self, block: i64) -> Result<ProverData, String>;
    fn publish(&self, p: groth16::Proof<models::node::Engine>) -> Result<(), String>;
}

pub fn start<'a, C: 'static + Sync + Send + ApiClient>(prover: Prover<C>) {
    let (tx_block_start, rx_block_start) = mpsc::channel();
    let prover = Arc::new(prover);
    let prover_rc = Arc::clone(&prover);
    let join_handle = thread::spawn(move || {
        prover.run_rounds(tx_block_start);
        println!("exit run_rounds.");
    });
    prover_rc.keep_sending_work_heartbeats(rx_block_start);
    println!("exit keep_sending_work_heartbeats");
    join_handle.join();
}

impl<C: ApiClient> Prover<C> {
    pub fn new(
        circuit_params: groth16::Parameters<bn256::Bn256>,
        jubjub_params: franklin_crypto::alt_babyjubjub::AltJubjubBn256,
        api_client: C,
        heartbeat_interval: time::Duration,
        stop_signal: Arc<AtomicBool>,
    ) -> Self {
        Prover {
            circuit_params,
            jubjub_params,
            api_client,
            heartbeat_interval,
            stop_signal,
        }
    }

    fn run_rounds(&self, start_heartbeats_tx: mpsc::Sender<Option<i64>>) {
        // TODO: add PROVER_CYCLE_WAIT usage
        let mut rng = rand::OsRng::new().unwrap();

        while !self.stop_signal.load(Ordering::SeqCst) {
            let block_to_prove = self.api_client.block_to_prove();
            if let Ok(block) = block_to_prove {
                // Notify heartbeat routine that work on new block has started.
                start_heartbeats_tx.send(block);

                if let Some(block) = block {
                    let prover_data = match self.api_client.prover_data(block) {
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

                    let proof = bellman::groth16::create_random_proof(
                        instance,
                        &self.circuit_params,
                        &mut rng,
                    );

                    if proof.is_err() {
                        // TODO: panic?
                        panic!("proof can not be created: {}", proof.err().unwrap());
                    }

                    // TODO: handle error.
                    let p = proof.unwrap();

                    self.api_client.publish(p);
                }
            }
        }
    }

    fn keep_sending_work_heartbeats(&self, start_heartbeats_rx: mpsc::Receiver<Option<i64>>) {
        let mut proving_block = 0;
        while !self.stop_signal.load(Ordering::SeqCst) {
            thread::sleep(self.heartbeat_interval);
            proving_block = match start_heartbeats_rx.try_recv() {
                Ok(Some(v)) => v,
                Err(mpsc::TryRecvError::Empty) => proving_block,
                Ok(None) => 0,
                _ => break,
            };
            if proving_block != 0 {
                self.api_client.working_on(proving_block);
            }
        }
    }
}
