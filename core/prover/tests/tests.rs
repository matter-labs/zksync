// Built-in deps
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::{thread, time};
// External deps
use crypto_exports::franklin_crypto::{self, bellman};
use crypto_exports::pairing::ff::PrimeField;
use num::BigUint;
// Workspace deps
use circuit::witness::deposit::{apply_deposit_tx, calculate_deposit_operations_from_witness};
use circuit::witness::utils::WitnessBuilder;
use models::node::operations::DepositOp;
use models::node::{Deposit, Engine, Fr};
use models::params::block_chunk_sizes;
use prover::prover_data::ProverData;
use prover::read_circuit_params;

#[test]
#[cfg_attr(not(feature = "keys-required"), ignore)]
fn prover_sends_heartbeat_requests_and_exits_on_stop_signal() {
    // Testing [black box] that:
    // - BabyProver sends `working_on` requests (heartbeat) over api client
    // - BabyProver stops running upon receiving data over stop channel

    let block_size_chunks = block_chunk_sizes()[0];

    // Create a channel to notify on provers exit.
    let (done_tx, done_rx) = mpsc::channel();
    // Create channel to notify test about heartbeat requests.
    let (heartbeat_tx, heartbeat_rx) = mpsc::channel();

    // Run prover in a separate thread.
    let stop_signal = Arc::new(AtomicBool::new(false));
    let stop_signal_ar = Arc::clone(&stop_signal);
    let circuit_parameters = read_circuit_params(block_size_chunks);
    let jubjub_params = franklin_crypto::alt_babyjubjub::AltJubjubBn256::new();
    thread::spawn(move || {
        // Create channel for proofs, not using in this test.
        let (tx, _) = mpsc::channel();
        let p = prover::BabyProver::new(
            circuit_parameters,
            jubjub_params,
            block_size_chunks,
            MockApiClient {
                block_to_prove: Mutex::new(Some((1, 1))),
                heartbeats_tx: Arc::new(Mutex::new(heartbeat_tx)),
                publishes_tx: Arc::new(Mutex::new(tx)),
                prover_data_fn: || None,
            },
            time::Duration::from_millis(100),
            stop_signal_ar,
        );
        let (tx, rx) = mpsc::channel();
        let jh = thread::spawn(move || {
            rx.recv().expect("on receive from exit error channel"); // mock receive exit error.
        });
        prover::start(p, tx);
        jh.join().expect("failed to join recv");
        done_tx.send(()).expect("unexpected failure");
    });

    let timeout = time::Duration::from_secs(10);

    // Must receive heartbeat requests.
    heartbeat_rx
        .recv_timeout(timeout)
        .expect("heartbeat request is not received");
    heartbeat_rx
        .recv_timeout(timeout)
        .expect("heartbeat request is not received");

    // Send stop signal.
    let jh = thread::spawn(move || {
        println!("waiting for first heartbeat");
        // receive at least one heartbeat.
        heartbeat_rx
            .recv_timeout(timeout)
            .expect("[heartbeat_rx] first heartbeat");
        while let Ok(_) = heartbeat_rx.recv_timeout(timeout) {}
        // BabyProver must be stopped.
        done_rx.recv_timeout(timeout).expect("[done_rx] recv");
    });
    stop_signal.store(true, Ordering::SeqCst);
    jh.join().expect("prover did not exit properly");
}

#[test]
#[cfg_attr(not(feature = "keys-required"), ignore)]
fn prover_proves_a_block_and_publishes_result() {
    // Testing [black box] the actual proof calculation by mocking genesis and +1 block.
    let stop_signal = Arc::new(AtomicBool::new(false));
    let (proof_tx, proof_rx) = mpsc::channel();
    let prover_data = new_test_data_for_prover();
    let block_size_chunks = prover_data.operations.len();

    let public_data_commitment = prover_data.public_data_commitment;
    let circuit_params = read_circuit_params(block_size_chunks);
    let verify_key = bellman::groth16::prepare_verifying_key(&circuit_params.vk);

    // Run prover in separate thread.
    let stop_signal_ar = Arc::clone(&stop_signal);
    let jubjub_params = franklin_crypto::alt_babyjubjub::AltJubjubBn256::new();
    thread::spawn(move || {
        // Work heartbeat channel, not used in this test.
        let (tx, _) = mpsc::channel();
        let p = prover::BabyProver::new(
            circuit_params,
            jubjub_params,
            block_size_chunks,
            MockApiClient {
                block_to_prove: Mutex::new(Some((1, 1))),
                heartbeats_tx: Arc::new(Mutex::new(tx)),
                publishes_tx: Arc::new(Mutex::new(proof_tx)),
                prover_data_fn: move || Some(prover_data.clone()),
            },
            time::Duration::from_secs(1),
            stop_signal_ar,
        );

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            rx.recv().unwrap();
        });
        prover::start(p, tx);
    });

    let timeout = time::Duration::from_secs(60 * 30);
    let proof = proof_rx
        .recv_timeout(timeout)
        .expect("didn't receive proof");
    stop_signal.store(true, Ordering::SeqCst);
    println!("verifying proof...");
    let verify_result =
        bellman::groth16::verify_proof(&verify_key, &proof, &[public_data_commitment]);
    assert!(!verify_result.is_err());
    assert!(verify_result.unwrap(), "invalid proof");
}

fn new_test_data_for_prover() -> ProverData {
    use circuit::witness::test_utils::test_genesis_plasma_state;
    let (_plasma_state, mut circuit_account_tree) = test_genesis_plasma_state(Vec::new());
    let fee_account_id = 0;
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, fee_account_id, 1);

    let empty_account_id = 1;
    let empty_account_address = [7u8; 20].into();
    let deposit_op = DepositOp {
        priority_op: Deposit {
            from: empty_account_address,
            token: 0,
            amount: BigUint::from(1u32),
            to: empty_account_address,
        },
        account_id: empty_account_id,
    };

    let deposit_witness = apply_deposit_tx(&mut witness_accum.account_tree, &deposit_op);
    let deposit_operations = calculate_deposit_operations_from_witness(&deposit_witness);
    let pub_data_from_witness = deposit_witness.get_pubdata();

    witness_accum.add_operation_with_pubdata(deposit_operations, pub_data_from_witness);
    witness_accum.extend_pubdata_with_noops();
    witness_accum.collect_fees(&Vec::new());
    witness_accum.calculate_pubdata_commitment();

    ProverData {
        public_data_commitment: witness_accum.pubdata_commitment.unwrap(),
        old_root: witness_accum.initial_root_hash,
        new_root: witness_accum.root_after_fees.unwrap(),
        validator_address: Fr::from_str(&witness_accum.fee_account_id.to_string())
            .expect("failed to parse"),
        operations: witness_accum.operations,
        validator_balances: witness_accum.fee_account_balances.unwrap(),
        validator_audit_path: witness_accum.fee_account_audit_path.unwrap(),
        validator_account: witness_accum.fee_account_witness.unwrap(),
    }
}

struct MockApiClient<F: Fn() -> Option<ProverData>> {
    block_to_prove: Mutex<Option<(i64, i32)>>,
    heartbeats_tx: Arc<Mutex<mpsc::Sender<()>>>,
    publishes_tx: Arc<Mutex<mpsc::Sender<bellman::groth16::Proof<Engine>>>>,
    prover_data_fn: F,
}

impl<F: Fn() -> Option<ProverData>> prover::ApiClient for MockApiClient<F> {
    fn block_to_prove(&self, _block_size: usize) -> Result<Option<(i64, i32)>, failure::Error> {
        let block_to_prove = self.block_to_prove.lock().unwrap();
        Ok(*block_to_prove)
    }

    fn working_on(&self, job: i32) -> Result<(), failure::Error> {
        let stored = self.block_to_prove.lock().unwrap();
        if let Some((_, stored)) = *stored {
            if stored != job {
                return Err(failure::format_err!("unexpected job id"));
            }
            let _ = self.heartbeats_tx.lock().unwrap().send(());
        }
        Ok(())
    }

    fn prover_data(&self, _block: i64) -> Result<ProverData, failure::Error> {
        let block_to_prove = self.block_to_prove.lock().unwrap();
        if (*block_to_prove).is_some() {
            let v = (self.prover_data_fn)();
            if let Some(pd) = v {
                return Ok(pd);
            }
        }
        Err(failure::format_err!("mock not configured"))
    }

    fn publish(
        &self,
        _block: i64,
        p: bellman::groth16::Proof<Engine>,
    ) -> Result<(), failure::Error> {
        // No more blocks to prove. We're only testing single rounds.
        let mut block_to_prove = self.block_to_prove.lock().unwrap();
        *block_to_prove = None;

        let _ = self.publishes_tx.lock().unwrap().send(p);
        Ok(())
    }
}
