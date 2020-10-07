// Built-in deps
use std::fmt;
use std::sync::{mpsc, Arc, Mutex};
use std::{thread, time};
// External deps
use num::BigUint;
use zksync_crypto::pairing::ff::PrimeField;
// Workspace deps
use zksync_circuit::{
    circuit::ZkSyncCircuit,
    witness::{deposit::DepositWitness, utils::WitnessBuilder, Witness},
};
use zksync_config::ConfigurationOptions;
use zksync_crypto::{
    circuit::{account::CircuitAccount, CircuitAccountTree},
    proof::EncodedProofPlonk,
    Engine, Fr,
};
use zksync_prover_utils::prover_data::ProverData;
use zksync_types::{
    block::smallest_block_size_for_chunks, operations::DepositOp, Account, Address, Deposit,
};
// Local deps
use zksync_prover::{
    plonk_step_by_step_prover::{PlonkStepByStepProver, PlonkStepByStepProverConfig},
    ProverImpl,
};

#[test]
#[cfg_attr(not(feature = "keys-required"), ignore)]
fn prover_sends_heartbeat_requests_and_exits_on_stop_signal() {
    // Testing [black box] that:
    // - BabyProver sends `working_on` requests (heartbeat) over api client
    // - BabyProver stops running upon receiving data over stop channel

    let block_size_chunks = ConfigurationOptions::from_env().available_block_chunk_sizes[0];

    // Create a channel to notify on provers exit.
    let (done_tx, _done_rx) = mpsc::channel();
    // Create channel to notify test about heartbeat requests.
    let (heartbeat_tx, heartbeat_rx) = mpsc::channel();

    // Run prover in a separate thread.
    thread::spawn(move || {
        // Create channel for proofs, not using in this test.
        let (tx, _) = mpsc::channel();
        let config = PlonkStepByStepProverConfig {
            block_sizes: vec![block_size_chunks],
            download_setup_from_network: false,
        };
        let p = PlonkStepByStepProver::create_from_config(
            config,
            MockApiClient {
                block_to_prove: Mutex::new(Some((1, 1))),
                heartbeats_tx: Arc::new(Mutex::new(heartbeat_tx)),
                publishes_tx: Arc::new(Mutex::new(tx)),
                prover_data_fn: || None,
            },
            time::Duration::from_millis(100),
        );
        let (tx, rx) = mpsc::channel();
        let jh = thread::spawn(move || {
            rx.recv().expect("on receive from exit error channel"); // mock receive exit error.
        });
        zksync_prover::start(p, tx, Default::default());
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
}

#[test]
#[cfg_attr(not(feature = "keys-required"), ignore)]
fn prover_proves_a_block_and_publishes_result() {
    // Testing [black box] the actual proof calculation by mocking genesis and +1 block.
    let (proof_tx, proof_rx) = mpsc::channel();
    let prover_data = new_test_data_for_prover();
    let block_size_chunks = prover_data.operations.len();

    // Run prover in separate thread.
    thread::spawn(move || {
        // Work heartbeat channel, not used in this test.
        let (tx, _) = mpsc::channel();
        let config = PlonkStepByStepProverConfig {
            block_sizes: vec![block_size_chunks],
            download_setup_from_network: false,
        };
        let p = PlonkStepByStepProver::create_from_config(
            config,
            MockApiClient {
                block_to_prove: Mutex::new(Some((1, 1))),
                heartbeats_tx: Arc::new(Mutex::new(tx)),
                publishes_tx: Arc::new(Mutex::new(proof_tx)),
                prover_data_fn: move || Some(prover_data.clone()),
            },
            time::Duration::from_secs(1),
        );

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            rx.recv().unwrap();
        });
        zksync_prover::start(p, tx, Default::default());
    });

    let timeout = time::Duration::from_secs(60 * 10);
    proof_rx
        .recv_timeout(timeout)
        .expect("didn't receive proof"); // if proof is received - then proof is verified
}

fn new_test_data_for_prover() -> ProverData {
    let mut circuit_account_tree =
        CircuitAccountTree::new(zksync_crypto::params::account_tree_depth());
    let fee_account_id = 0;

    // Init the fee account.
    let fee_account = Account::default_with_address(&Address::default());
    circuit_account_tree.insert(fee_account_id, CircuitAccount::from(fee_account));

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

    let deposit_witness = DepositWitness::apply_tx(&mut witness_accum.account_tree, &deposit_op);
    let deposit_operations = deposit_witness.calculate_operations(());
    let pub_data_from_witness = deposit_witness.get_pubdata();

    witness_accum.add_operation_with_pubdata(deposit_operations, pub_data_from_witness);
    witness_accum.extend_pubdata_with_noops(smallest_block_size_for_chunks(
        DepositOp::CHUNKS,
        &ConfigurationOptions::from_env().available_block_chunk_sizes,
    ));
    witness_accum.collect_fees(&Vec::new());
    witness_accum.calculate_pubdata_commitment();

    ProverData {
        public_data_commitment: witness_accum.pubdata_commitment.unwrap(),
        old_root: witness_accum.initial_root_hash,
        initial_used_subtree_root: witness_accum.initial_used_subtree_root_hash,
        new_root: witness_accum.root_after_fees.unwrap(),
        validator_address: Fr::from_str(&witness_accum.fee_account_id.to_string())
            .expect("failed to parse"),
        operations: witness_accum.operations,
        validator_balances: witness_accum.fee_account_balances.unwrap(),
        validator_audit_path: witness_accum.fee_account_audit_path.unwrap(),
        validator_account: witness_accum.fee_account_witness.unwrap(),
    }
}

struct MockApiClient<F> {
    block_to_prove: Mutex<Option<(i64, i32)>>,
    heartbeats_tx: Arc<Mutex<mpsc::Sender<()>>>,
    publishes_tx: Arc<Mutex<mpsc::Sender<EncodedProofPlonk>>>,
    prover_data_fn: F,
}

impl<F> fmt::Debug for MockApiClient<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MockApiClient").finish()
    }
}

impl<F: Fn() -> Option<ProverData>> zksync_prover::ApiClient for MockApiClient<F> {
    fn block_to_prove(&self, _block_size: usize) -> Result<Option<(i64, i32)>, anyhow::Error> {
        let block_to_prove = self.block_to_prove.lock().unwrap();
        Ok(*block_to_prove)
    }

    fn working_on(&self, job: i32) -> Result<(), anyhow::Error> {
        let stored = self.block_to_prove.lock().unwrap();
        if let Some((_, stored)) = *stored {
            if stored != job {
                return Err(anyhow::format_err!("unexpected job id"));
            }
            let _ = self.heartbeats_tx.lock().unwrap().send(());
        }
        Ok(())
    }

    fn prover_data(&self, block: i64) -> Result<ZkSyncCircuit<'_, Engine>, anyhow::Error> {
        let block_to_prove = self.block_to_prove.lock().unwrap();
        if (*block_to_prove).is_some() {
            let v = (self.prover_data_fn)();
            if let Some(pd) = v {
                return Ok(pd.into_circuit(block));
            }
        }
        Err(anyhow::format_err!("mock not configured"))
    }

    fn publish(&self, _block: i64, p: EncodedProofPlonk) -> Result<(), anyhow::Error> {
        // No more blocks to prove. We're only testing single rounds.
        let mut block_to_prove = self.block_to_prove.lock().unwrap();
        *block_to_prove = None;

        let _ = self.publishes_tx.lock().unwrap().send(p);
        Ok(())
    }

    fn prover_stopped(&self, _: i32) -> Result<(), anyhow::Error> {
        Ok(())
    }
}
