#[macro_use]
extern crate log;

use rand::OsRng;
use std::fmt;
use std::iter::Iterator;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

use bellman::groth16::{
    create_random_proof, prepare_verifying_key, verify_proof, Parameters, Proof,
};
use circuit::account::AccountWitness;
use circuit::circuit::FranklinCircuit;
use circuit::witness::close_account::*;
use circuit::witness::deposit::*;
use circuit::witness::noop::noop_operation;
use circuit::witness::transfer::*;
use circuit::witness::transfer_to_new::*;
use circuit::witness::utils::*;
use circuit::witness::withdraw::*;
use ff::{Field, PrimeField};
use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use franklin_crypto::jubjub::JubjubEngine;
use models::circuit::account::CircuitAccount;
use models::circuit::CircuitAccountTree;
use models::node::Account;
use models::node::*;
use models::params as franklin_constants;
use models::primitives::pack_bits_into_bytes_in_order;
use models::EncodedProof;
use plasma::state::PlasmaState;
use tokio::prelude::*;
use tokio::runtime::current_thread::Handle;
use tokio::sync::oneshot::Sender;
use tokio::timer;

use num_traits::cast::ToPrimitive;
// use models::circuit::encoder;
// use models::config::{
//    DEPOSIT_BATCH_SIZE, EXIT_BATCH_SIZE, PROVER_CYCLE_WAIT, PROVER_TIMEOUT, PROVER_TIMER_TICK,
//    RUNTIME_CONFIG,
// };
use storage::StorageProcessor;

use models::primitives::{serialize_g1_for_ethereum, serialize_g2_for_ethereum};

pub struct Prover<E: JubjubEngine> {
    pub operation_batch_size: usize,
    pub current_block_number: BlockNumber,
    pub accounts_tree: CircuitAccountTree,
    pub parameters: BabyParameters,
    pub jubjub_params: E::Params,
    pub worker: String,
    pub prover_id: i32,
    pub current_job: Arc<AtomicUsize>,
}

pub type BabyProof = Proof<Engine>;
pub type BabyParameters = Parameters<Engine>;
pub type BabyProver = Prover<Engine>;

#[derive(Debug)]
pub enum BabyProverErr {
    InvalidAmountEncoding,
    InvalidFeeEncoding,
    InvalidSender,
    InvalidRecipient,
    InvalidTransaction(String),
    IoError(std::io::Error),
    Other(String),
}

impl BabyProverErr {
    fn description(&self) -> String {
        match *self {
            BabyProverErr::InvalidAmountEncoding => {
                "transfer amount is malformed or too large".to_owned()
            }
            BabyProverErr::InvalidFeeEncoding => {
                "transfer fee is malformed or too large".to_owned()
            }
            BabyProverErr::InvalidSender => "sender account is unknown".to_owned(),
            BabyProverErr::InvalidRecipient => "recipient account is unknown".to_owned(),
            BabyProverErr::InvalidTransaction(ref reason) => format!("invalid tx data: {}", reason),
            BabyProverErr::IoError(_) => "encountered an I/O error".to_owned(),
            BabyProverErr::Other(ref reason) => format!("Prover error: {}", reason),
        }
    }
}

impl fmt::Display for BabyProverErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if let BabyProverErr::IoError(ref e) = *self {
            write!(f, "I/O error: ")?;
            e.fmt(f)
        } else {
            write!(f, "{}", self.description())
        }
    }
}

#[derive(Debug)]
pub struct FullBabyProof {
    proof: BabyProof,
    inputs: [Fr; 1],
    public_data: Vec<u8>,
}

fn read_parameters(file_name: &str) -> Result<BabyParameters, BabyProverErr> {
    use std::fs::File;
    use std::io::BufReader;

    let f_r = File::open(file_name);
    if f_r.is_err() {
        return Err(BabyProverErr::IoError(f_r.err().unwrap()));
    }
    let mut r = BufReader::new(f_r.unwrap());
    let circuit_params = BabyParameters::read(&mut r, true);

    if circuit_params.is_err() {
        return Err(BabyProverErr::IoError(circuit_params.err().unwrap()));
    }

    Ok(circuit_params.unwrap())
}

fn extend_accounts<I: Sized + Iterator<Item = (AccountId, Account)>>(
    tree: &mut CircuitAccountTree,
    accounts: I,
) {
    for e in accounts {
        let acc_number = e.0;
        let leaf_copy = CircuitAccount::from(e.1.clone());
        tree.insert(acc_number, leaf_copy);
    }
}

// IMPORTANT: prover does NOT care about some ordering of the transactions, so blocks supplied here MUST be ordered
// for the application layer

impl BabyProver {
    // Outputs
    // - 8 uint256 for encoding of the field elements
    // - one uint256 for new root hash
    // - uint256 block number
    // - uint256 total fees
    // - Bytes public data
    //
    // Old root is available to take from the storage of the smart-contract
    pub fn encode_proof(proof: &FullBabyProof) -> Result<EncodedProof, Err> {
        // proof
        // pub a: E::G1Affine,
        // pub b: E::G2Affine,
        // pub c: E::G1Affine

        let (a_x, a_y) = serialize_g1_for_ethereum(proof.proof.a);

        let ((b_x_0, b_x_1), (b_y_0, b_y_1)) = serialize_g2_for_ethereum(proof.proof.b);

        let (c_x, c_y) = serialize_g1_for_ethereum(proof.proof.c);

        // let new_root = serialize_fe_for_ethereum(proof.inputs[1]);

        // let total_fees = serialize_fe_for_ethereum(proof.total_fees);

        // let block_number = serialize_fe_for_ethereum(proof.block_number);

        // let public_data = proof.public_data.clone();

        let p = [a_x, a_y, b_x_0, b_x_1, b_y_0, b_y_1, c_x, c_y];

        // EncodedProof{
        //     groth_proof: [a_x, a_y, b_x_0, b_x_1, b_y_0, b_y_1, c_x, c_y],
        //     //block_number: block_number,
        // };

        Ok(p)
    }

    pub fn create(worker: String) -> Result<BabyProver, BabyProverErr> {
        let storage =
            StorageProcessor::establish_connection().expect("db connection failed for prover");
        let (last_block, accounts) = storage
            .load_verified_state()
            .expect("db must be functional");
        info!("Last block is: {}", last_block);
        debug!("Accounts: {:?}", accounts);
        let initial_state = PlasmaState::new(accounts, last_block);

        info!("Reading proving key, may take a while");

        let path = {
            let mut key_file_path = std::path::PathBuf::new();
            key_file_path.push(&std::env::var("KEY_DIR").expect("KEY_DIR not set"));
            key_file_path.push(&format!("{}", franklin_constants::BLOCK_SIZE_CHUNKS));
            key_file_path.push(franklin_constants::KEY_FILENAME);
            key_file_path
        };
        debug!("Reading key from {}", path.to_str().unwrap());
        let franklin_circuit_params = read_parameters(&path.to_str().unwrap());
        if franklin_circuit_params.is_err() {
            return Err(franklin_circuit_params.err().unwrap());
        }

        debug!("Done reading franklin key");

        info!("Copying states to balance tree");

        // TODO: replace with .clone() by moving PedersenHasher to static context
        let mut tree = CircuitAccountTree::new(franklin_constants::ACCOUNT_TREE_DEPTH as u32);
        extend_accounts(&mut tree, initial_state.get_accounts().into_iter());

        let root = tree.root_hash();

        let state_block_number = initial_state.block_number;

        info!(
            "Initial root hash is {} for block {}",
            root, state_block_number
        );

        let supplied_root = initial_state.root_hash();
        info!("supplied_root is: {}", supplied_root);
        if root != supplied_root {
            return Err(BabyProverErr::Other("root did not change".to_owned()));
        }

        let jubjub_params = AltJubjubBn256::new();

        let prover_id = storage
            .register_prover(&worker)
            .expect("getting prover id failed");

        Ok(Self {
            operation_batch_size: franklin_constants::BLOCK_SIZE_CHUNKS,
            current_block_number: state_block_number,
            accounts_tree: tree,
            parameters: franklin_circuit_params.unwrap(),
            jubjub_params,
            current_job: Arc::new(AtomicUsize::new(0)),
            worker,
            prover_id,
        })
    }
}

type Err = BabyProverErr;

impl BabyProver {
    fn rewind_state(
        &mut self,
        storage: &StorageProcessor,
        expected_current_block: BlockNumber,
    ) -> Result<(), String> {
        info!(
            "rewinding the state from block #{} to #{}",
            self.current_block_number, expected_current_block
        );
        let (_, new_accounts) = storage
            .load_committed_state(Some(expected_current_block))
            .map_err(|e| format!("load_state_diff failed: {}", e))?;

        let mut tree = CircuitAccountTree::new(franklin_constants::ACCOUNT_TREE_DEPTH as u32);
        extend_accounts(&mut tree, new_accounts.into_iter());

        self.accounts_tree = tree;
        self.current_block_number = expected_current_block;
        Ok(())
    }

    fn make_proving_attempt(&mut self) -> Result<(), String> {
        let storage = StorageProcessor::establish_connection()
            .map_err(|e| format!("establish_connection failed: {}", e))?;
        let job = storage
            .fetch_prover_job(&self.worker, config::PROVER_TIMEOUT)
            .map_err(|e| format!("fetch_prover_job failed: {}", e))?;

        if let Some(job) = job {
            let block_number = job.block_number as BlockNumber;
            info!(
                "prover {} got a new job for block {}",
                &self.worker, block_number
            );
            self.current_job.store(job.id as usize, Ordering::Relaxed);

            // load state delta self.current_block_number => block_number (can go both forwards and backwards)
            let expected_current_block = block_number - 1;
            if self.current_block_number != expected_current_block {
                self.rewind_state(&storage, expected_current_block)?;
            }
            let initial_root = self.accounts_tree.root_hash();

            for (index, item) in &self.accounts_tree.items {
                info!("index: {}, item: {}", index, item.pub_key_hash);
            }
            let block = storage
                .load_committed_block(block_number)
                .ok_or("load_committed_block failed")?;
            let ops = storage.get_block_operations(block.block_number).unwrap();

            drop(storage);
            let mut operations = vec![];
            let mut pub_data = vec![];
            let mut fees = vec![];
            for op in ops {
                match op {
                    FranklinOp::Deposit(deposit) => {
                        let deposit_witness = apply_deposit_tx(&mut self.accounts_tree, &deposit);

                        //assert!(tree.verify_proof(sender_leaf_number, sender_leaf.clone(), tree.merkle_path(sender_leaf_number)));
                        let (signature, sig_msg, sender_x, sender_y) = generate_dummy_sig_data();
                        let deposit_operations = calculate_deposit_operations_from_witness(
                            &deposit_witness,
                            &sig_msg,
                            signature,
                            &sender_x,
                            &sender_y,
                        );
                        operations.extend(deposit_operations);
                        fees.push((deposit.tx.fee, deposit.tx.token));
                        pub_data.extend(deposit_witness.get_pubdata());
                    }
                    FranklinOp::Transfer(transfer) => {
                        let transfer_witness =
                            apply_transfer_tx(&mut self.accounts_tree, &transfer);
                        let (signature, sig_msg, sender_x, sender_y) = generate_dummy_sig_data();
                        let transfer_operations = calculate_transfer_operations_from_witness(
                            &transfer_witness,
                            &sig_msg,
                            signature,
                            &sender_x,
                            &sender_y,
                        );
                        operations.extend(transfer_operations);
                        fees.push((transfer.tx.fee, transfer.tx.token));
                        pub_data.extend(transfer_witness.get_pubdata());
                    }
                    FranklinOp::TransferToNew(transfer_to_new) => {
                        let transfer_to_new_witness =
                            apply_transfer_to_new_tx(&mut self.accounts_tree, &transfer_to_new);
                        let (signature, sig_msg, sender_x, sender_y) = generate_dummy_sig_data();
                        let transfer_to_new_operations =
                            calculate_transfer_to_new_operations_from_witness(
                                &transfer_to_new_witness,
                                &sig_msg,
                                signature,
                                &sender_x,
                                &sender_y,
                            );
                        operations.extend(transfer_to_new_operations);
                        fees.push((transfer_to_new.tx.fee, transfer_to_new.tx.token));
                        pub_data.extend(transfer_to_new_witness.get_pubdata());
                    }
                    FranklinOp::Withdraw(withdraw) => {
                        let withdraw_witness =
                            apply_withdraw_tx(&mut self.accounts_tree, &withdraw);
                        let (signature, sig_msg, sender_x, sender_y) = generate_dummy_sig_data();
                        let withdraw_operations = calculate_withdraw_operations_from_witness(
                            &withdraw_witness,
                            &sig_msg,
                            signature,
                            &sender_x,
                            &sender_y,
                        );
                        operations.extend(withdraw_operations);
                        fees.push((withdraw.tx.fee, withdraw.tx.token));
                        pub_data.extend(withdraw_witness.get_pubdata());
                    }
                    FranklinOp::Close(close) => {
                        let close_account_witness =
                            apply_close_account_tx(&mut self.accounts_tree, &close);
                        let (signature, sig_msg, sender_x, sender_y) = generate_dummy_sig_data();
                        let close_account_operations =
                            calculate_close_account_operations_from_witness(
                                &close_account_witness,
                                &sig_msg,
                                signature,
                                &sender_x,
                                &sender_y,
                            );
                        operations.extend(close_account_operations);
                        pub_data.extend(close_account_witness.get_pubdata());
                    }
                }
            }
            if operations.len() < franklin_constants::BLOCK_SIZE_CHUNKS {
                for _ in 0..franklin_constants::BLOCK_SIZE_CHUNKS - operations.len() {
                    let (signature, sig_msg, sender_x, sender_y) = generate_dummy_sig_data();
                    operations.push(noop_operation(
                        &self.accounts_tree,
                        block.fee_account,
                        &sig_msg,
                        signature,
                        &sender_x,
                        &sender_y,
                    ));
                    pub_data.extend(vec![false; 64]);
                }
            }
            assert_eq!(pub_data.len(), 64 * franklin_constants::BLOCK_SIZE_CHUNKS);
            assert_eq!(operations.len(), franklin_constants::BLOCK_SIZE_CHUNKS);

            let validator_acc = self
                .accounts_tree
                .get(block.fee_account as u32)
                .expect("fee_account is not empty");
            let mut validator_balances = vec![];
            for i in 0..1 << franklin_constants::BALANCE_TREE_DEPTH {
                //    validator_balances.push(Some(validator_acc.subtree.get(i as u32).map(|s| s.clone()).unwrap_or(Balance::default())));
                let balance_value = match validator_acc.subtree.get(i as u32) {
                    None => Fr::zero(),
                    Some(bal) => bal.value,
                };
                validator_balances.push(Some(balance_value));
            }
            let mut root_after_fee: Fr = self.accounts_tree.root_hash();
            let mut validator_account_witness: AccountWitness<Engine> = AccountWitness {
                nonce: None,
                pub_key_hash: None,
            };
            for (fee, token) in fees {
                info!("fee, token: {}, {}", fee, token);
                let (root, acc_witness) = apply_fee(
                    &mut self.accounts_tree,
                    block.fee_account,
                    u32::from(token),
                    fee.to_u128().unwrap(),
                );
                root_after_fee = root;
                validator_account_witness = acc_witness;
            }

            info!("root after fees {}", root_after_fee);
            info!("block new hash {}", block.new_root_hash);
            assert_eq!(root_after_fee, block.new_root_hash);
            let (validator_audit_path, _) =
                get_audits(&self.accounts_tree, block.fee_account as u32, 0);

            info!("Data for public commitment. pub_data: {:x?}, initial_root: {}, final_root: {}, validator_address: {}, block_number: {}",
                  pack_bits_into_bytes_in_order(pub_data.clone()), initial_root, root_after_fee, Fr::from_str(&block.fee_account.to_string()).unwrap(), Fr::from_str(&(block_number + 1).to_string()).unwrap()
                );
            let public_data_commitment = public_data_commitment::<Engine>(
                &pub_data,
                Some(initial_root),
                Some(root_after_fee),
                Some(Fr::from_str(&block.fee_account.to_string()).unwrap()),
                Some(Fr::from_str(&(block_number).to_string()).unwrap()),
            );

            let instance = FranklinCircuit {
                params: &self.jubjub_params,
                operation_batch_size: franklin_constants::BLOCK_SIZE_CHUNKS,
                old_root: Some(initial_root),
                new_root: Some(block.new_root_hash),
                block_number: Fr::from_str(&(block_number).to_string()),
                validator_address: Some(Fr::from_str(&block.fee_account.to_string()).unwrap()),
                pub_data_commitment: Some(public_data_commitment),
                operations: operations.clone(),
                validator_balances: validator_balances.clone(),
                validator_audit_path: validator_audit_path.clone(),
                validator_account: validator_account_witness.clone(),
            };

            // {
            //     let inst = FranklinCircuit {
            //         params: &self.jubjub_params,
            //         operation_batch_size: franklin_constants::BLOCK_SIZE_CHUNKS,
            //         old_root: Some(initial_root),
            //         new_root: Some(block.new_root_hash),
            //         block_number: Fr::from_str(&(block_number + 1).to_string()),
            //         validator_address: Some(Fr::from_str(&block.fee_account.to_string()).unwrap()),
            //         pub_data_commitment: Some(public_data_commitment.clone()),
            //         operations: operations,
            //         validator_balances: validator_balances,
            //         validator_audit_path: validator_audit_path,
            //         validator_account: validator_account_witness,
            //     };
            //     let mut cs = TestConstraintSystem::<Engine>::new();
            //     inst.synthesize(&mut cs).unwrap();

            //     warn!("unconstrained {}\n", cs.find_unconstrained());
            //     warn!("inputs {}\n", cs.num_inputs());
            //     warn!("num_constraints: {}\n", cs.num_constraints());
            //     warn!("is satisfied: {}\n", cs.is_satisfied());
            //     warn!("which is unsatisfied: {:?}\n", cs.which_is_unsatisfied());
            // }

            let mut rng = OsRng::new().unwrap();
            info!("Prover has started to work");
            // let tmp_cirtuit_params = generate_random_parameters(instance, &mut rng).unwrap();
            let proof = create_random_proof(instance, &self.parameters, &mut rng);
            if proof.is_err() {
                error!("proof can not be created: {}", proof.err().unwrap());
                return Err("proof can not be created".to_owned());
                //             return Err(BabyProverErr::Other("proof.is_err()".to_owned()));
            }
            let p = proof.unwrap();

            let pvk = prepare_verifying_key(&self.parameters.vk);

            info!(
                "Made a proof for initial root = {}, final root = {}, public_data_commitment = {}",
                initial_root,
                root_after_fee,
                public_data_commitment.to_hex()
            );
            let success = verify_proof(&pvk, &p.clone(), &[public_data_commitment]);
            if success.is_err() {
                error!(
                    "Proof is verification failed with error {}",
                    success.err().unwrap()
                );
                return Err("Proof verification has failed".to_owned());
                //             return Err(BabyProverErr::Other(
                //                 "Proof is verification failed".to_owned(),
                //             ));
            }
            if !success.unwrap() {
                error!("Proof is invalid");
                return Err("Proof is invalid".to_owned());
                //             return Err(BabyProverErr::Other("Proof is invalid".to_owned()));
            }

            info!("Proof generation is complete");

            let full_proof = FullBabyProof {
                proof: p,
                inputs: [public_data_commitment],
                // public_data: pub_data,
                public_data: vec![0 as u8; 10],
            };

            //        Ok(full_proof)

            let encoded = Self::encode_proof(&full_proof).expect("proof encoding failed");
            let storage = StorageProcessor::establish_connection()
                .map_err(|e| format!("establish_connection failed: {}", e))?;
            storage
                .store_proof(block_number, &encoded)
                .map_err(|e| format!("store_proof failed: {}", e))?;
        } else {
            thread::sleep(Duration::from_secs(config::PROVER_CYCLE_WAIT));
        }
        Ok(())
    }

    pub fn start_timer_interval(&self, rt: &Handle) {
        let job_ref = self.current_job.clone();
        rt.spawn(
            timer::Interval::new_interval(Duration::from_secs(config::PROVER_TIMER_TICK))
                .fold(job_ref, |job_ref, _| {
                    let job = job_ref.load(Ordering::Relaxed);
                    if job > 0 {
                        //debug!("prover is working on block {}", job);
                        if let Ok(storage) = StorageProcessor::establish_connection() {
                            let _ = storage.update_prover_job(job as i32);
                        }
                    }
                    Ok(job_ref)
                })
                .map(|_| ())
                .map_err(|_| ()),
        )
        .unwrap();
    }

    pub fn run(&mut self, shutdown_tx: Sender<()>, stop_signal: Arc<AtomicBool>) {
        info!("prover is running");
        while !stop_signal.load(Ordering::SeqCst) {
            if let Err(err) = self.make_proving_attempt() {
                error!("Error: {}", err);
            }
            self.current_job.store(0, Ordering::Relaxed);
        }
        info!("prover stopped");
        shutdown_tx.send(()).unwrap();
    }
}
