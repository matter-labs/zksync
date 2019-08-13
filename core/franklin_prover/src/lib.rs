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

use bellman::{Circuit, ConstraintSystem, SynthesisError};
use circuit::account::AccountWitness;
use circuit::circuit::FranklinCircuit;
use circuit::operation::Operation;
use circuit::tests::deposit::*;
use circuit::tests::noop::noop_operation;
use circuit::tests::utils::*;
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use ff::{BitIterator, Field, PrimeField, PrimeFieldRepr};
use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use franklin_crypto::circuit::float_point::parse_float_to_u128;
use franklin_crypto::circuit::test::TestConstraintSystem;
use franklin_crypto::jubjub::{edwards, JubjubEngine};
use models::node::config::*;
use models::node::operations::*;
use models::node::Account;
use models::params as franklin_constants;
use models::EncodedProof;
use rustc_hex::ToHex;
use storage::ProverRun;
use tokio::prelude::*;
use tokio::runtime::current_thread::Handle;
use tokio::sync::oneshot::Sender;
use tokio::timer;
use bellman::groth16::generate_random_parameters;
use bellman::groth16::{
    create_random_proof, prepare_verifying_key, verify_proof, Parameters, Proof,
};
use models::circuit::account::{Balance, CircuitAccount};
use models::circuit::CircuitAccountTree;
use models::merkle_tree::*;
use models::node::block::{Block, ExecutedTx};
use models::node::*;
use models::primitives::*;
use plasma::state::PlasmaState;

use num_traits::cast::ToPrimitive;
// use models::circuit::encoder;
// use models::config::{
//    DEPOSIT_BATCH_SIZE, EXIT_BATCH_SIZE, PROVER_CYCLE_WAIT, PROVER_TIMEOUT, PROVER_TIMER_TICK,
//    RUNTIME_CONFIG,
// };
use storage::StorageProcessor;

use circuit::utils::be_bit_vector_into_bytes;

// use circuit::transfer::circuit::{TransactionWitness, Transfer};
use circuit::operation::*;

use models::primitives::{
    field_element_to_u32, serialize_g1_for_ethereum, serialize_g2_for_ethereum,
};

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

        //    let keys_path = &RUNTIME_CONFIG.keys_path;

        //    let path = format!("{}/transfer_pk.key", keys_path);

        let path = format!("core/franklin_key_generator/franklin_pk.key");
        debug!("Reading key from {}", path);
        let franklin_circuit_params = read_parameters(&path);
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
        let mut job = storage
            .fetch_prover_job(&self.worker, PROVER_TIMEOUT)
            .map_err(|e| format!("fetch_prover_job failed: {}", e))?;

        if let Some(job) = job {
            let initial_root = self.accounts_tree.root_hash();
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
                    _ => {
                        unreachable!("only deposists allowed");
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
            assert_eq!(pub_data.len(), 64 * 10);
            assert_eq!(operations.len(), 10);

            let validator_acc = self
                .accounts_tree
                .get(block.fee_account as u32)
                .expect("fee_account is not empty");
            let mut validator_balances = vec![];
            for i in 0..1 << *franklin_constants::BALANCE_TREE_DEPTH {
                //    validator_balances.push(Some(validator_acc.subtree.get(i as u32).map(|s| s.clone()).unwrap_or(Balance::default())));
                let balance_value = match validator_acc.subtree.get(i as u32) {
                    None => Fr::zero(),
                    Some(bal) => bal.value.clone(),
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
                    block.fee_account as u32,
                    token as u32,
                    fee.to_u128().unwrap(),
                );
                root_after_fee = root;
                validator_account_witness = acc_witness;
            }

            info!("root after fees {}", root_after_fee);
            info!("block new hash {}", block.new_root_hash);
            assert_eq!(root_after_fee, block.new_root_hash);

            let (validator_audit_path, _) =
                get_audits(&mut self.accounts_tree, block.fee_account as u32, 0);

            let public_data_commitment = public_data_commitment::<Engine>(
                &pub_data,
                Some(initial_root.clone()),
                Some(root_after_fee.clone()),
                Some(Fr::from_str(&block.fee_account.to_string()).unwrap()),
                Some(Fr::from_str(&(block_number + 1).to_string()).unwrap()),
            );
       
            let instance = FranklinCircuit {
                    params: &self.jubjub_params,
                    operation_batch_size: franklin_constants::BLOCK_SIZE_CHUNKS,
                    old_root: Some(initial_root),
                    new_root: Some(block.new_root_hash),
                    block_number: Fr::from_str(&(block_number + 1).to_string()),
                    validator_address: Some(Fr::from_str(&block.fee_account.to_string()).unwrap()),
                    pub_data_commitment: Some(public_data_commitment.clone()),
                    operations: operations.clone(),
                    validator_balances: validator_balances.clone(),
                    validator_audit_path: validator_audit_path.clone(),
                    validator_account: validator_account_witness.clone(),
                };
      
            {
                let inst = FranklinCircuit {
                    params: &self.jubjub_params,
                    operation_batch_size: franklin_constants::BLOCK_SIZE_CHUNKS,
                    old_root: Some(initial_root),
                    new_root: Some(block.new_root_hash),
                    block_number: Fr::from_str(&(block_number + 1).to_string()),
                    validator_address: Some(Fr::from_str(&block.fee_account.to_string()).unwrap()),
                    pub_data_commitment: Some(public_data_commitment.clone()),
                    operations: operations,
                    validator_balances: validator_balances,
                    validator_audit_path: validator_audit_path,
                    validator_account: validator_account_witness,
                };
                let mut cs = TestConstraintSystem::<Engine>::new();
                inst.synthesize(&mut cs).unwrap();

                warn!("unconstrained {}\n", cs.find_unconstrained());
                warn!("inputs {}\n", cs.num_inputs());
                warn!("num_constraints: {}\n", cs.num_constraints());
                warn!("is satisfied: {}\n", cs.is_satisfied());
                warn!("which is unsatisfied: {:?}\n", cs.which_is_unsatisfied());
            }

            let mut rng = OsRng::new().unwrap();
            info!("Prover has started to work");
            // let tmp_cirtuit_params = generate_random_parameters(instance, &mut rng).unwrap();
            self.parameters.vk.alpha_g1.len()
            let proof = create_random_proof(instance, &self.parameters, &mut rng);
            if proof.is_err() {
                error!("proof can not be created: {}", proof.err().unwrap());
                return Err("proof can not be created".to_owned());
                //             return Err(BabyProverErr::Other("proof.is_err()".to_owned()));
            }
            let p = proof.unwrap();

            let pvk = prepare_verifying_key(&self.parameters.vk);

            info!(
                "Made a proof for initial root = {}, final root = {}, public data = {}",
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
            // no new job, so let's try to fast forward to the latest verified state for efficiency, and then sleep
            let last_verified_block = storage
                .get_last_verified_block()
                .map_err(|e| format!("get_last_verified_block failed: {}", e))?;
            if self.current_block_number < last_verified_block + 1 {
                self.rewind_state(&storage, last_verified_block + 1)
                    .map_err(|e| format!("rewind_state failed: {}", e))?;
            }
            thread::sleep(Duration::from_secs(PROVER_CYCLE_WAIT));
        }
        Ok(())
    }

    pub fn start_timer_interval(&self, rt: &Handle) {
        let job_ref = self.current_job.clone();
        rt.spawn(
            timer::Interval::new_interval(Duration::from_secs(PROVER_TIMER_TICK))
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

//    // Apply transactions to the state while also making a witness for proof, then calculate proof
//    pub fn apply_and_prove_transfer(
//        &mut self,
//        block: &Block,
//        transactions: &[TransferTx],
//    ) -> Result<FullBabyProof, Err> {
//        let block_number = block.block_number;
//        if block_number != self.current_block_number {
//            debug!(
//                "Transfer proof request is for block {}, while prover state is block {}",
//                block_number, self.current_block_number
//            );
//            return Err(BabyProverErr::Other("incorrect block".to_owned()));
//        }
//        let block_final_root = block.new_root_hash;

//        let public_data: Vec<u8> =
//            encoder::encode_transactions(&block).expect("encoding transactions failed");

//        //let transactions = &block.transactions;
//        let num_txes = transactions.len();

//        if num_txes != self.transfer_batch_size {
//            return Err(BabyProverErr::Other(
//                "num_txes != self.transfer_batch_size".to_owned(),
//            ));
//        }

//        let mut witnesses: Vec<(Transaction<Engine>, TransactionWitness<Engine>)> = vec![];

//        let mut total_fees = Fr::zero();

//        let initial_root = self.accounts_tree.root_hash();

//        for tx in transactions {
//            let tx = circuit::CircuitTransferTx::try_from(tx)
//                .map_err(|e| BabyProverErr::InvalidTransaction(e.to_string()))?;
//            let sender_leaf_number = field_element_to_u32(tx.from);
//            let recipient_leaf_number = field_element_to_u32(tx.to);

//            let empty_account = CircuitAccount::default();

//            let tree = &mut self.accounts_tree;
//            let items = tree.items.clone();

//            let sender_leaf = items.get(&sender_leaf_number);
//            let mut recipient_leaf = items.get(&recipient_leaf_number);

//            if sender_leaf.is_none() {
//                return Err(BabyProverErr::InvalidSender);
//            }

//            // allow transfers to empty accounts
//            if recipient_leaf.is_none() {
//                recipient_leaf = Some(&empty_account);
//            }

//            // this is LE bits encoding of the transaction amount
//            let mut amount_bits: Vec<bool> = BitIterator::new(tx.amount.into_repr()).collect();
//            amount_bits.reverse();
//            amount_bits
//                .truncate(params::AMOUNT_EXPONENT_BIT_WIDTH + params::AMOUNT_MANTISSA_BIT_WIDTH);

//            let parsed_transfer_amount = parse_float_to_u128(
//                amount_bits,
//                params::AMOUNT_EXPONENT_BIT_WIDTH,
//                params::AMOUNT_MANTISSA_BIT_WIDTH,
//                10,
//            );

//            // this is LE bits encoding of the transaction fee
//            let mut fee_bits: Vec<bool> = BitIterator::new(tx.fee.into_repr()).collect();
//            fee_bits.reverse();
//            fee_bits.truncate(params::FEE_EXPONENT_BIT_WIDTH + params::FEE_MANTISSA_BIT_WIDTH);

//            let parsed_fee = parse_float_to_u128(
//                fee_bits,
//                params::FEE_EXPONENT_BIT_WIDTH,
//                params::FEE_MANTISSA_BIT_WIDTH,
//                10,
//            );

//            if parsed_transfer_amount.is_err() || parsed_fee.is_err() {
//                return Err(BabyProverErr::InvalidAmountEncoding);
//            }

//            let transfer_amount_as_field_element =
//                Fr::from_str(&parsed_transfer_amount.unwrap().to_string()).unwrap();
//            let fee_as_field_element = Fr::from_str(&parsed_fee.unwrap().to_string()).unwrap();

//            let path_from: Vec<Option<Fr>> = tree
//                .merkle_path(sender_leaf_number)
//                .into_iter()
//                .map(|e| Some(e.0))
//                .collect();
//            let path_to: Vec<Option<Fr>> = tree
//                .merkle_path(recipient_leaf_number)
//                .into_iter()
//                .map(|e| Some(e.0))
//                .collect();

//            let transaction = Transaction {
//                from: Some(tx.from),
//                to: Some(tx.to),
//                amount: Some(tx.amount),
//                fee: Some(tx.fee),
//                nonce: Some(tx.nonce),
//                good_until_block: Some(tx.good_until_block),
//                signature: Some(tx.signature.clone()),
//            };

//            let mut updated_sender_leaf = sender_leaf.unwrap().clone();
//            let mut updated_recipient_leaf = recipient_leaf.unwrap().clone();

//            updated_sender_leaf
//                .balance
//                .sub_assign(&transfer_amount_as_field_element);
//            updated_sender_leaf
//                .balance
//                .sub_assign(&fee_as_field_element);

//            updated_sender_leaf.nonce.add_assign(&Fr::one());

//            if recipient_leaf_number != 0 {
//                updated_recipient_leaf
//                    .balance
//                    .add_assign(&transfer_amount_as_field_element);
//            }

//            total_fees.add_assign(&fee_as_field_element);

//            tree.insert(sender_leaf_number, updated_sender_leaf.clone());
//            tree.insert(recipient_leaf_number, updated_recipient_leaf.clone());

//            {
//                let sender_leaf = sender_leaf.unwrap();

//                let recipient_leaf = recipient_leaf.unwrap();

//                let transaction_witness = TransactionWitness::<Engine> {
//                    auth_path_from: path_from,
//                    leaf_from: LeafWitness::<Engine> {
//                        balance: Some(sender_leaf.balance),
//                        nonce: Some(sender_leaf.nonce),
//                        pub_x: Some(sender_leaf.pub_x),
//                        pub_y: Some(sender_leaf.pub_y),
//                    },
//                    auth_path_to: path_to,
//                    leaf_to: LeafWitness::<Engine> {
//                        balance: Some(recipient_leaf.balance),
//                        nonce: Some(recipient_leaf.nonce),
//                        pub_x: Some(recipient_leaf.pub_x),
//                        pub_y: Some(recipient_leaf.pub_y),
//                    },
//                };

//                let witness = (transaction.clone(), transaction_witness);

//                witnesses.push(witness);
//            }
//        }

//        let block_number = Fr::from_str(&block_number.to_string()).unwrap();

//        let final_root = self.accounts_tree.root_hash();

//        if initial_root == final_root {
//            return Err(BabyProverErr::Other(
//                "initial_root == final_root".to_owned(),
//            ));
//        }

//        info!(
//            "Prover final root = {}, final root from state keeper = {}",
//            final_root, block_final_root
//        );

//        if block_final_root != final_root {
//            return Err(BabyProverErr::Other(
//                "block_final_root != final_root".to_owned(),
//            ));
//        }

//        self.current_block_number += 1;

//        let mut public_data_initial_bits = vec![];

//        // these two are BE encodings because an iterator is BE. This is also an Ethereum standard behavior

//        let block_number_bits: Vec<bool> = BitIterator::new(block_number.into_repr()).collect();
//        for _ in 0..256 - block_number_bits.len() {
//            public_data_initial_bits.push(false);
//        }
//        public_data_initial_bits.extend(block_number_bits.into_iter());

//        let total_fee_bits: Vec<bool> = BitIterator::new(total_fees.into_repr()).collect();
//        for _ in 0..256 - total_fee_bits.len() {
//            public_data_initial_bits.push(false);
//        }
//        public_data_initial_bits.extend(total_fee_bits.into_iter());

//        assert_eq!(public_data_initial_bits.len(), 512);

//        let mut h = Sha256::new();

//        let bytes_to_hash = be_bit_vector_into_bytes(&public_data_initial_bits);

//        // let hex_block_and_fee: String = bytes_to_hash.clone().to_hex();
//        // debug!("Packed initial hash information = {}", hex_block_and_fee);

//        h.input(&bytes_to_hash);

//        let mut hash_result = [0u8; 32];
//        h.result(&mut hash_result[..]);

//        {
//            let packed_transaction_data_bytes = public_data.clone();

//            // let hex: String = packed_transaction_data_bytes.clone().to_hex();
//            // debug!("Packed transfers information data = {}", hex);

//            let mut next_round_hash_bytes = Vec::new();
//            next_round_hash_bytes.extend(hash_result.iter());
//            next_round_hash_bytes.extend(packed_transaction_data_bytes);

//            let mut h = Sha256::new();

//            h.input(&next_round_hash_bytes);

//            h.result(&mut hash_result[..]);
//        }

//        // clip to fit into field element

//        hash_result[0] &= 0x1f; // temporary solution

//        let mut repr = Fr::zero().into_repr();
//        repr.read_be(&hash_result[..])
//            .expect("pack hash as field element");

//        let public_data_commitment = Fr::from_repr(repr).unwrap();

//        let instance = Transfer {
//            params: &self.jubjub_params,
//            number_of_transactions: num_txes,
//            old_root: Some(initial_root),
//            new_root: Some(final_root),
//            public_data_commitment: Some(public_data_commitment),
//            block_number: Some(block_number),
//            total_fee: Some(total_fees),
//            transactions: witnesses.clone(),
//        };

//        // {
//        //     let inst = Transfer {
//        //         params: &self.jubjub_params,
//        //         number_of_transactions: num_txes,
//        //         old_root: Some(initial_root),
//        //         new_root: Some(final_root),
//        //         public_data_commitment: Some(public_data_commitment),
//        //         block_number: Some(block_number),
//        //         total_fee: Some(total_fees),
//        //         transactions: witnesses.clone(),
//        //     };

//        //     use franklin_crypto::circuit::test::*;
//        //     use bellman::Circuit;
//        //     let mut cs = TestConstraintSystem::<Engine>::new();
//        //     inst.synthesize(&mut cs).unwrap();

//        //     print!("{}\n", cs.find_unconstrained());

//        //     print!("{}\n", cs.num_constraints());

//        //     assert_eq!(cs.num_inputs(), 4);

//        //     let err = cs.which_is_unsatisfied();
//        //     if err.is_some() {
//        //         panic!("ERROR satisfying in {}\n", err.unwrap());
//        //     }
//        //     debug!("CS is satisfied!");
//        // }

//        let mut rng = OsRng::new().unwrap();
//        info!("Prover has started to work transfer");
//        let proof = create_random_proof(instance, &self.transfer_parameters, &mut rng);
//        if proof.is_err() {
//            return Err(BabyProverErr::Other("proof.is_err()".to_owned()));
//        }

//        let p = proof.unwrap();

//        let pvk = prepare_verifying_key(&self.transfer_parameters.vk);

//        info!(
//            "Made a proof for initial root = {}, final root = {}, public data = {}",
//            initial_root,
//            final_root,
//            public_data_commitment.to_hex()
//        );
//        let success = verify_proof(
//            &pvk,
//            &p.clone(),
//            &[initial_root, final_root, public_data_commitment],
//        );
//        if success.is_err() {
//            error!(
//                "Proof is verification failed with error {}",
//                success.err().unwrap()
//            );
//            return Err(BabyProverErr::Other(
//                "Proof is verification failed".to_owned(),
//            ));
//        }
//        if !success.unwrap() {
//            error!("Proof is invalid");
//            return Err(BabyProverErr::Other("Proof is invalid".to_owned()));
//        }

//        info!("Proof generation is complete");

//        let full_proof = FullBabyProof {
//            proof: p,
//            inputs: [initial_root, final_root, public_data_commitment],
//            total_fees,
//            block_number,
//            public_data,
//        };

//        Ok(full_proof)
//    }

//    // expects accounts in block to be sorted already
//    pub fn apply_and_prove_deposit(
//        &mut self,
//        block: &Block,
//        transactions: &[DepositTx],
//    ) -> Result<FullBabyProof, Err> {
//        // debug!("block: {:?}", &block.block_data);
//        // debug!("transactions: {:?}", &transactions);

//        let block_number = block.block_number;
//        if block_number != self.current_block_number {
//            error!(
//                "Deposit proof request is for block {}, while prover state is block {}",
//                block_number, self.current_block_number
//            );
//            return Err(BabyProverErr::Other(
//                "block_number != self.current_block_number".to_owned(),
//            ));
//        }
//        let block_final_root = block.new_root_hash;

//        let public_data: Vec<u8> =
//            encoder::encode_transactions(block).expect("prover: encoding failed");

//        //let transactions = &block.transactions;
//        let num_txes = transactions.len();

//        if num_txes != self.deposit_batch_size {
//            return Err(BabyProverErr::Other(
//                "num_txes != self.deposit_batch_size".to_owned(),
//            ));
//        }

//        let mut witnesses: Vec<(DepositRequest<Engine>, DepositWitness<Engine>)> = vec![];

//        let initial_root = self.accounts_tree.root_hash();

//        for tx in transactions {
//            let tx = circuit::CircuitDepositRequest::try_from(tx)
//                .map_err(|e| BabyProverErr::InvalidTransaction(e.to_string()))?;

//            let into_leaf_number = field_element_to_u32(tx.into);

//            let tree = &mut self.accounts_tree;
//            let items = tree.items.clone();

//            let existing_leaf = items.get(&into_leaf_number);
//            let mut leaf_is_empty = true;

//            let (old_leaf, new_leaf) = if existing_leaf.is_none() {
//                let mut new_leaf = CircuitAccount::default();
//                new_leaf.balance = tx.amount;
//                new_leaf.pub_x = tx.pub_x;
//                new_leaf.pub_y = tx.pub_y;

//                (CircuitAccount::default(), new_leaf)
//            } else {
//                let old_leaf = existing_leaf.unwrap().clone();
//                let mut new_leaf = old_leaf.clone();
//                new_leaf.balance.add_assign(&tx.amount);
//                leaf_is_empty = false;

//                (old_leaf, new_leaf)
//            };

//            let path: Vec<Option<Fr>> = tree
//                .merkle_path(into_leaf_number)
//                .into_iter()
//                .map(|e| Some(e.0))
//                .collect();

//            let public_key =
//                edwards::Point::from_xy(new_leaf.pub_x, new_leaf.pub_y, &self.jubjub_params);

//            if public_key.is_none() {
//                return Err(BabyProverErr::Other("public_key.is_none()".to_owned()));
//            }

//            let request = DepositRequest {
//                into: Fr::from_str(&into_leaf_number.to_string()),
//                amount: Some(tx.amount),
//                public_key,
//            };

//            tree.insert(into_leaf_number, new_leaf.clone());

//            {
//                let deposit_witness = DepositWitness::<Engine> {
//                    auth_path: path,
//                    leaf: LeafWitness::<Engine> {
//                        balance: Some(old_leaf.balance),
//                        nonce: Some(old_leaf.nonce),
//                        pub_x: Some(old_leaf.pub_x),
//                        pub_y: Some(old_leaf.pub_y),
//                    },

//                    leaf_is_empty: Some(leaf_is_empty),
//                    new_pub_x: Some(new_leaf.pub_x),
//                    new_pub_y: Some(new_leaf.pub_y),
//                };

//                let witness = (request.clone(), deposit_witness);

//                witnesses.push(witness);
//            }
//        }

//        let block_number = Fr::from_str(&block_number.to_string()).unwrap();

//        let final_root = self.accounts_tree.root_hash();

//        info!(
//            "Prover final root = {}, final root from state keeper = {}",
//            final_root, block_final_root
//        );

//        if initial_root == final_root {
//            return Err(BabyProverErr::Other(format!(
//                "initial_root == final_root, {:?}",
//                initial_root
//            )));
//        }

//        if block_final_root != final_root {
//            return Err(BabyProverErr::Other(
//                "block_final_root != final_root".to_owned(),
//            ));
//        }

//        self.current_block_number += 1;

//        let mut public_data_initial_bits = vec![];

//        // these two are BE encodings because an iterator is BE. This is also an Ethereum standard behavior

//        let block_number_bits: Vec<bool> = BitIterator::new(block_number.into_repr()).collect();
//        for _ in 0..256 - block_number_bits.len() {
//            public_data_initial_bits.push(false);
//        }
//        public_data_initial_bits.extend(block_number_bits.into_iter());

//        assert_eq!(public_data_initial_bits.len(), 256);

//        let mut h = Sha256::new();

//        let bytes_to_hash = be_bit_vector_into_bytes(&public_data_initial_bits);

//        let hex_block_and_fee: String = bytes_to_hash.clone().to_hex();
//        debug!(
//            "Packed initial hash information in deposit = {}",
//            hex_block_and_fee
//        );

//        h.input(&bytes_to_hash);

//        let mut hash_result = [0u8; 32];
//        h.result(&mut hash_result[..]);

//        let initial_hash: String = hash_result.to_hex();
//        debug!("Block number hash in deposit = {}", initial_hash);

//        {
//            let packed_transaction_data_bytes = public_data.clone();

//            let hex: String = packed_transaction_data_bytes.clone().to_hex();
//            debug!("Packed deposit information data in deposit = {}", hex);

//            let mut next_round_hash_bytes = vec![];
//            next_round_hash_bytes.extend(hash_result.iter());
//            next_round_hash_bytes.extend(packed_transaction_data_bytes);

//            let mut h = Sha256::new();

//            h.input(&next_round_hash_bytes);

//            h.result(&mut hash_result[..]);
//        }

//        // clip to fit into field element

//        hash_result[0] &= 0x1f; // temporary solution

//        let mut repr = Fr::zero().into_repr();
//        repr.read_be(&hash_result[..])
//            .expect("pack hash as field element");

//        let public_data_commitment = Fr::from_repr(repr).unwrap();

//        let instance = Deposit {
//            params: &self.jubjub_params,
//            number_of_deposits: num_txes,
//            old_root: Some(initial_root),
//            new_root: Some(final_root),
//            public_data_commitment: Some(public_data_commitment),
//            block_number: Some(block_number),
//            requests: witnesses.clone(),
//        };

//        let mut rng = OsRng::new().unwrap();
//        debug!("Prover has started to work deposits");
//        let proof = create_random_proof(instance, &self.deposit_parameters, &mut rng);
//        if proof.is_err() {
//            return Err(BabyProverErr::Other("proof.is_err()".to_owned()));
//        }

//        let p = proof.unwrap();

//        let pvk = prepare_verifying_key(&self.deposit_parameters.vk);

//        debug!(
//            "Made an deposit proof for initial root = {}, final root = {}, public data = {}",
//            initial_root,
//            final_root,
//            public_data_commitment.to_hex()
//        );
//        let success = verify_proof(
//            &pvk,
//            &p.clone(),
//            &[initial_root, final_root, public_data_commitment],
//        );

//        if success.is_err() {
//            error!(
//                "Proof verification failed with error {}",
//                success.err().unwrap()
//            );
//            return Err(BabyProverErr::Other("Proof verification failed".to_owned()));
//        }
//        if !success.unwrap() {
//            error!("Proof is invalid");
//            return Err(BabyProverErr::Other("Proof is invalid".to_owned()));
//        }
//        info!("Proof generation is complete");

//        let full_proof = FullBabyProof {
//            proof: p,
//            inputs: [initial_root, final_root, public_data_commitment],
//            total_fees: Fr::zero(),
//            block_number,
//            public_data,
//        };

//        Ok(full_proof)
//    }

//    // expects accounts in block to be sorted already
//    pub fn apply_and_prove_exit(
//        &mut self,
//        block: &Block,
//        transactions: &[ExitTx],
//    ) -> Result<FullBabyProof, Err> {
//        let block_number = block.block_number;
//        if block_number != self.current_block_number {
//            info!(
//                "Exit proof request is for block {}, while prover state is block {}",
//                block_number, self.current_block_number
//            );
//            return Err(BabyProverErr::Other(
//                "block_number != self.current_block_number".to_owned(),
//            ));
//        }
//        let block_final_root = block.new_root_hash;

//        //let transactions = &block.transactions;
//        let num_txes = transactions.len();

//        if num_txes != self.deposit_batch_size {
//            return Err(BabyProverErr::Other(
//                "num_txes != self.deposit_batch_size".to_owned(),
//            ));
//        }

//        let mut witnesses: Vec<(ExitRequest<Engine>, ExitWitness<Engine>)> = Vec::new();

//        let initial_root = self.accounts_tree.root_hash();

//        // we also need to create public data cause we need info from state
//        let mut public_data: Vec<u8> = Vec::new();

//        for tx in transactions {
//            let tx = circuit::CircuitExitRequest::try_from(tx)
//                .map_err(|e| BabyProverErr::InvalidTransaction(e.to_string()))?;

//            let from_leaf_number = field_element_to_u32(tx.from);

//            let tree = &mut self.accounts_tree;
//            let items = tree.items.clone();

//            let existing_leaf = items.get(&from_leaf_number);

//            if existing_leaf.is_none() {
//                return Err(BabyProverErr::Other("existing_leaf.is_none()".to_owned()));
//            }

//            let old_leaf = existing_leaf.unwrap();

//            let new_leaf = CircuitAccount::default();

//            let path: Vec<Option<Fr>> = tree
//                .merkle_path(from_leaf_number)
//                .into_iter()
//                .map(|e| Some(e.0))
//                .collect();

//            let request = ExitRequest {
//                from: Fr::from_str(&from_leaf_number.to_string()),
//                amount: Some(old_leaf.balance),
//            };

//            // we have the leaf info, so add it to the public data
//            let tx_bits = request.public_data_into_bits();
//            let tx_encoding = be_bit_vector_into_bytes(&tx_bits);
//            public_data.extend(tx_encoding.into_iter());

//            tree.insert(from_leaf_number, new_leaf.clone());

//            {
//                let deposit_witness = ExitWitness::<Engine> {
//                    auth_path: path,
//                    leaf: LeafWitness::<Engine> {
//                        balance: Some(old_leaf.balance),
//                        nonce: Some(old_leaf.nonce),
//                        pub_x: Some(old_leaf.pub_x),
//                        pub_y: Some(old_leaf.pub_y),
//                    },
//                };

//                let witness = (request.clone(), deposit_witness);

//                witnesses.push(witness);
//            }
//        }

//        let block_number = Fr::from_str(&block_number.to_string()).unwrap();

//        let final_root = self.accounts_tree.root_hash();

//        if initial_root == final_root {
//            return Err(BabyProverErr::Other(
//                "initial_root == final_root".to_owned(),
//            ));
//        }

//        debug!(
//            "Prover final root = {}, final root from state keeper = {}",
//            final_root, block_final_root
//        );

//        if block_final_root != final_root {
//            return Err(BabyProverErr::Other(
//                "block_final_root != final_root".to_owned(),
//            ));
//        }

//        self.current_block_number += 1;

//        let mut public_data_initial_bits = vec![];

//        // these two are BE encodings because an iterator is BE. This is also an Ethereum standard behavior

//        let block_number_bits: Vec<bool> = BitIterator::new(block_number.into_repr()).collect();
//        for _ in 0..256 - block_number_bits.len() {
//            public_data_initial_bits.push(false);
//        }
//        public_data_initial_bits.extend(block_number_bits.into_iter());

//        assert_eq!(public_data_initial_bits.len(), 256);

//        let mut h = Sha256::new();

//        let bytes_to_hash = be_bit_vector_into_bytes(&public_data_initial_bits);

//        // let hex_block_and_fee: String = bytes_to_hash.clone().to_hex();
//        // debug!("Packed initial hash information in exit = {}", hex_block_and_fee);

//        h.input(&bytes_to_hash);

//        let mut hash_result = [0u8; 32];
//        h.result(&mut hash_result[..]);

//        // let initial_hash: String = hash_result.clone().to_hex();
//        // debug!("Block number hash in exit = {}", initial_hash);

//        {
//            let packed_transaction_data_bytes = public_data.clone();

//            // let hex: String = packed_transaction_data_bytes.clone().to_hex();
//            // debug!("Packed transfers information data in exit= {}", hex);

//            let mut next_round_hash_bytes = Vec::new();
//            next_round_hash_bytes.extend(hash_result.iter());
//            next_round_hash_bytes.extend(packed_transaction_data_bytes);

//            // let hex_full: String = next_round_hash_bytes.clone().to_hex();
//            // debug!("Final hashable information data in exit= {}", hex_full);

//            let mut h = Sha256::new();

//            h.input(&next_round_hash_bytes);

//            h.result(&mut hash_result[..]);
//        }

//        // clip to fit into field element

//        // let final_hash_hex: String = hash_result.clone().to_hex();
//        // debug!("Full public data commitment = {}", final_hash_hex);

//        hash_result[0] &= 0x1f; // temporary solution

//        let mut repr = Fr::zero().into_repr();
//        repr.read_be(&hash_result[..])
//            .expect("pack hash as field element");

//        let public_data_commitment = Fr::from_repr(repr).unwrap();

//        let empty_leaf_witness = LeafWitness::<Engine> {
//            balance: Some(Fr::zero()),
//            nonce: Some(Fr::zero()),
//            pub_x: Some(Fr::zero()),
//            pub_y: Some(Fr::zero()),
//        };

//        let instance = Exit {
//            params: &self.jubjub_params,
//            number_of_exits: num_txes,
//            old_root: Some(initial_root),
//            new_root: Some(final_root),
//            public_data_commitment: Some(public_data_commitment),
//            empty_leaf_witness,
//            block_number: Some(block_number),
//            requests: witnesses.clone(),
//        };

//        let mut rng = OsRng::new().unwrap();
//        debug!("Prover has started to work on exits");
//        let proof = create_random_proof(instance, &self.exit_parameters, &mut rng);
//        if proof.is_err() {
//            return Err(BabyProverErr::Other("proof.is_err()".to_owned()));
//        }

//        let p = proof.unwrap();

//        let pvk = prepare_verifying_key(&self.exit_parameters.vk);

//        info!(
//            "Made an exit proof for initial root = {}, final root = {}, public data = {}",
//            initial_root,
//            final_root,
//            public_data_commitment.to_hex()
//        );
//        let success = verify_proof(
//            &pvk,
//            &p.clone(),
//            &[initial_root, final_root, public_data_commitment],
//        );

//        if success.is_err() {
//            error!(
//                "Proof verification failed with error {}",
//                success.err().unwrap()
//            );
//            return Err(BabyProverErr::Other("Proof verification failed".to_owned()));
//        }
//        if !success.unwrap() {
//            error!("Proof is invalid");
//            return Err(BabyProverErr::Other("Proof is invalid".to_owned()));
//        }
//        info!("Proof generation is complete");

//        let full_proof = FullBabyProof {
//            proof: p,
//            inputs: [initial_root, final_root, public_data_commitment],
//            total_fees: Fr::zero(),
//            block_number,
//            public_data,
//        };

//        Ok(full_proof)
//    }

// #[test]

// fn test_exit_encoding() {
//     extern crate BigDecimal;
//     use models::plasma::ExitTx;
//     use self::BigDecimal::from_primitive;
//     let exit_tx = ExitTx {
//         account: 2,
//         amount: BigDecimal::from(1000),
//     }
//     let exitBlock = ExitBlock {

//     }
// }
