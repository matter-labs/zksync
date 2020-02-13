// Built-in deps
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::{env, fs, io, path, thread, time};
// External deps
use ff::{Field, PrimeField};
// Workspace deps
use prover;
use testhelper::TestAccount;

#[test]
#[cfg_attr(not(feature = "keys-required"), ignore)]
fn prover_sends_heartbeat_requests_and_exits_on_stop_signal() {
    // Testing [black box] that:
    // - BabyProver sends `working_on` requests (heartbeat) over api client
    // - BabyProver stops running upon receiving data over stop channel

    // Create a channel to notify on provers exit.
    let (done_tx, done_rx) = mpsc::channel();
    // Create channel to notify test about heartbeat requests.
    let (heartbeat_tx, heartbeat_rx) = mpsc::channel();

    // Run prover in a separate thread.
    let stop_signal = Arc::new(AtomicBool::new(false));
    let stop_signal_ar = Arc::clone(&stop_signal);
    let circuit_parameters = read_circuit_parameters();
    let jubjub_params = franklin_crypto::alt_babyjubjub::AltJubjubBn256::new();
    thread::spawn(move || {
        // Create channel for proofs, not using in this test.
        let (tx, _) = mpsc::channel();
        let p = prover::BabyProver::new(
            circuit_parameters,
            jubjub_params,
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
        thread::spawn(move || {
            rx.recv().unwrap(); // mock receive exit error.
        });
        prover::start(p, tx);
        println!("run exited!");
        done_tx.send(()).expect("unexpected failure");
    });

    let timeout = time::Duration::from_millis(500);

    // Must receive heartbeat requests.
    heartbeat_rx
        .recv_timeout(timeout)
        .expect("heartbeat request is not received");
    heartbeat_rx
        .recv_timeout(timeout)
        .expect("heartbeat request is not received");

    // Send stop signal.
    let jh = thread::spawn(move || {
        println!("waiting for first hearbeat");
        heartbeat_rx.recv_timeout(timeout).unwrap();
        // BabyProver must be stopped.
        done_rx.recv_timeout(timeout).unwrap();
    });
    println!("sending stop signal.");
    stop_signal.store(true, Ordering::SeqCst);
    jh.join().expect("did not exit properly");
}

#[test]
#[cfg_attr(not(feature = "keys-required"), ignore)]
fn prover_proves_a_block_and_publishes_result() {
    // Testing [black box] the actual proof calculation by mocking genesis and +1 block.
    let circuit_params = read_circuit_parameters();
    let verify_key = bellman::groth16::prepare_verifying_key(&circuit_params.vk);
    let stop_signal = Arc::new(AtomicBool::new(false));
    let (proof_tx, proof_rx) = mpsc::channel();
    let prover_data = new_test_data_for_prover();
    let public_data_commitment = prover_data.public_data_commitment;

    // Run prover in separate thread.
    let stop_signal_ar = Arc::clone(&stop_signal);
    let jubjub_params = franklin_crypto::alt_babyjubjub::AltJubjubBn256::new();
    thread::spawn(move || {
        // Work heartbeat channel, not used in this test.
        let (tx, _) = mpsc::channel();
        let p = prover::BabyProver::new(
            circuit_params,
            jubjub_params,
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

    let timeout = time::Duration::from_secs(60 * 30); // 10 minutes
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

fn new_test_data_for_prover() -> prover::prover_data::ProverData {
    let mut circuit_tree =
        models::circuit::CircuitAccountTree::new(models::params::account_tree_depth() as u32);
    println!("Empty tree root hash: {}", circuit_tree.root_hash());

    let validator_test_account = TestAccount::new();
    println!(
        "validator account address: {:x}",
        validator_test_account.address
    );

    // Fee account
    let mut accounts = models::node::AccountMap::default();
    let mut validator_account = models::node::Account::default();
    validator_account.address = validator_test_account.address;
    let validator_account_id: u32 = 0;
    accounts.insert(validator_account_id, validator_account.clone());

    let mut state = plasma::state::PlasmaState::new(accounts, 1);
    let genesis_root_hash = state.root_hash();
    println!("Genesis block root hash: {}", genesis_root_hash);
    circuit_tree.insert(
        0,
        models::circuit::account::CircuitAccount::from(validator_account),
    );
    assert_eq!(circuit_tree.root_hash(), genesis_root_hash);

    let deposit_priority_op = models::node::FranklinPriorityOp::Deposit(models::node::Deposit {
        from: web3::types::Address::zero(),
        token: 0,
        amount: bigdecimal::BigDecimal::from(10),
        to: validator_test_account.address,
    });
    let mut op_success = state.execute_priority_op(deposit_priority_op.clone());
    let mut fees = Vec::new();
    let mut ops = Vec::new();
    let mut accounts_updated = Vec::new();

    if let Some(fee) = op_success.fee {
        fees.push(fee);
    }

    accounts_updated.append(&mut op_success.updates);

    ops.push(models::node::ExecutedOperations::PriorityOp(Box::new(
        models::node::ExecutedPriorityOp {
            op: op_success.executed_op,
            priority_op: models::node::PriorityOp {
                serial_id: 0,
                data: deposit_priority_op.clone(),
                deadline_block: 2,
                eth_fee: bigdecimal::BigDecimal::from(0),
                eth_hash: vec![0; 8],
            },
            block_index: 0,
        },
    )));

    let fee_updates = state.collect_fee(&fees, validator_account_id);
    accounts_updated.extend(fee_updates.into_iter());

    let block = models::node::block::Block {
        block_number: state.block_number,
        new_root_hash: state.root_hash(),
        fee_account: validator_account_id,
        block_transactions: ops,
        processed_priority_ops: (0, 1),
    };
    println!("Block: {:?}", block);

    let mut pub_data = vec![];
    let mut operations = vec![];

    if let models::node::FranklinPriorityOp::Deposit(deposit_op) = deposit_priority_op {
        let deposit_witness = circuit::witness::deposit::apply_deposit_tx(
            &mut circuit_tree,
            &models::node::operations::DepositOp {
                priority_op: deposit_op,
                account_id: 0,
            },
        );

        let deposit_operations =
            circuit::witness::deposit::calculate_deposit_operations_from_witness(
                &deposit_witness,
                &models::node::Fr::zero(),
                &models::node::Fr::zero(),
                &models::node::Fr::zero(),
                &circuit::operation::SignatureData {
                    r_packed: vec![Some(false); 256],
                    s: vec![Some(false); 256],
                },
                &[Some(false); 256],
            );
        operations.extend(deposit_operations);
        pub_data.extend(deposit_witness.get_pubdata());
    }

    let phaser = models::merkle_tree::PedersenHasher::<models::node::Engine>::default();
    let jubjub_params = &franklin_crypto::alt_babyjubjub::AltJubjubBn256::new();
    for _ in 0..models::params::block_size_chunks() - operations.len() {
        let (signature, first_sig_msg, second_sig_msg, third_sig_msg, _a, _b) =
            circuit::witness::utils::generate_dummy_sig_data(&[false], &phaser, &jubjub_params);

        operations.push(circuit::witness::noop::noop_operation(
            &circuit_tree,
            block.fee_account,
            &first_sig_msg,
            &second_sig_msg,
            &third_sig_msg,
            &signature,
            &[Some(false); 256],
        ));
        pub_data.extend(vec![false; 64]);
    }
    assert_eq!(pub_data.len(), 64 * models::params::block_size_chunks());
    assert_eq!(operations.len(), models::params::block_size_chunks());

    let validator_acc = circuit_tree
        .get(block.fee_account as u32)
        .expect("fee_account is not empty");
    let mut validator_balances = vec![];
    for i in 0..1 << models::params::BALANCE_TREE_DEPTH {
        let balance_value = match validator_acc.subtree.get(i as u32) {
            None => models::node::Fr::zero(),
            Some(bal) => bal.value,
        };
        validator_balances.push(Some(balance_value));
    }
    let _: models::node::Fr = circuit_tree.root_hash();
    let (root_after_fee, validator_account_witness) =
        circuit::witness::utils::apply_fee(&mut circuit_tree, block.fee_account, 0, 0);

    println!("root after fees {}", root_after_fee);
    println!("block new hash {}", block.new_root_hash);
    assert_eq!(root_after_fee, block.new_root_hash);
    let (validator_audit_path, _) =
        circuit::witness::utils::get_audits(&circuit_tree, block.fee_account as u32, 0);

    let public_data_commitment =
        circuit::witness::utils::public_data_commitment::<models::node::Engine>(
            &pub_data,
            Some(genesis_root_hash),
            Some(root_after_fee),
            Some(models::node::Fr::from_str(&block.fee_account.to_string()).unwrap()),
            Some(models::node::Fr::from_str(&(block.block_number).to_string()).unwrap()),
        );

    prover::prover_data::ProverData {
        public_data_commitment,
        old_root: genesis_root_hash,
        new_root: block.new_root_hash,
        validator_address: models::node::Fr::from_str(&block.fee_account.to_string()).unwrap(),
        operations,
        validator_balances,
        validator_audit_path,
        validator_account: validator_account_witness,
    }
}

fn read_circuit_parameters() -> bellman::groth16::Parameters<models::node::Engine> {
    let out_dir = {
        let mut out_dir = path::PathBuf::new();
        out_dir.push(&env::var("ZKSYNC_HOME").expect("ZKSYNC_HOME is not set"));
        out_dir.push(&env::var("KEY_DIR").expect("KEY_DIR is not set"));
        out_dir.push(&format!("{}", models::params::block_size_chunks()));
        out_dir.push(&format!("{}", models::params::account_tree_depth()));
        out_dir
    };
    let key_file_path = {
        let mut key_file_path = out_dir;
        key_file_path.push(models::params::KEY_FILENAME);
        key_file_path
    };
    println!("key file path is {:?}", key_file_path);
    let f = fs::File::open(&key_file_path).expect("Unable to open file");
    let mut r = io::BufReader::new(f);
    bellman::groth16::Parameters::<models::node::Engine>::read(&mut r, true)
        .expect("Unable to read proving key")
}

struct MockApiClient<F: Fn() -> Option<prover::prover_data::ProverData>> {
    block_to_prove: Mutex<Option<(i64, i32)>>,
    heartbeats_tx: Arc<Mutex<mpsc::Sender<()>>>,
    publishes_tx: Arc<Mutex<mpsc::Sender<bellman::groth16::Proof<models::node::Engine>>>>,
    prover_data_fn: F,
}

impl<F: Fn() -> Option<prover::prover_data::ProverData>> prover::ApiClient for MockApiClient<F> {
    fn block_to_prove(&self) -> Result<Option<(i64, i32)>, failure::Error> {
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

    fn prover_data(&self, _block: i64) -> Result<prover::prover_data::ProverData, failure::Error> {
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
        p: bellman::groth16::Proof<models::node::Engine>,
        _public_data_commitment: models::node::Fr,
    ) -> Result<(), failure::Error> {
        // No more blocks to prove. We're only testing single rounds.
        let mut block_to_prove = self.block_to_prove.lock().unwrap();
        *block_to_prove = None;

        let _ = self.publishes_tx.lock().unwrap().send(p);
        Ok(())
    }
}
