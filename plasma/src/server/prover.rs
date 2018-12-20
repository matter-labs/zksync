use std::error::Error;
use std::fmt;
use rand::{OsRng};
use std::sync::mpsc::{self, Sender, Receiver};

use crypto::sha2::Sha256;
use crypto::digest::Digest;

use ff::{Field, PrimeField, PrimeFieldRepr, BitIterator};
use crate::models::{Engine, Fr};
use sapling_crypto::circuit::float_point::parse_float_to_u128;
use sapling_crypto::alt_babyjubjub::{AltJubjubBn256};
use sapling_crypto::jubjub::{JubjubEngine, JubjubParams, edwards, Unknown};

use bellman::groth16::{Proof, Parameters, create_random_proof, verify_proof, prepare_verifying_key};

use crate::models::{self, params, TransferBlock, DepositBlock, ExitBlock, Block, PlasmaState};
use crate::models::circuit::{Account, AccountTree};

use super::config::{TRANSFER_BATCH_SIZE, DEPOSIT_BATCH_SIZE, EXIT_BATCH_SIZE};

use super::committer::{self, EncodedProof, BlockProof};

use crate::circuit::utils::be_bit_vector_into_bytes;
use crate::circuit::transfer::transaction::{Transaction};
use crate::circuit::leaf::{LeafWitness};
use crate::circuit::deposit::deposit_request::{DepositRequest};
use crate::circuit::deposit::circuit::{Deposit, DepositWitness};
use crate::circuit::exit::exit_request::{ExitRequest};
use crate::circuit::exit::circuit::{Exit, ExitWitness};

use crate::circuit::transfer::circuit::{
    Transfer, 
    TransactionWitness};

use crate::primitives::{serialize_g1_for_ethereum, serialize_g2_for_ethereum, serialize_fe_for_ethereum, field_element_to_u32};

use web3::types::{U256};

pub struct Prover<E:JubjubEngine> {
    pub transfer_batch_size: usize,
    pub deposit_batch_size: usize,
    pub exit_batch_size: usize,
    pub block_number: u32,
    pub accounts_tree: AccountTree,
    pub transfer_parameters: BabyParameters,
    pub deposit_parameters: BabyParameters,
    pub exit_parameters: BabyParameters,
    pub jubjub_params: E::Params,
}

pub type BabyProof = Proof<Engine>;
pub type BabyParameters = Parameters<Engine>;
pub type BabyProver = Prover<Engine>;

#[derive(Debug)]
pub enum BabyProverErr {
    Unknown,
    InvalidAmountEncoding,
    InvalidFeeEncoding,
    InvalidSender,
    InvalidRecipient,
    InvalidTransaction(String),
    IoError(std::io::Error)
}

impl Error for BabyProverErr {
    fn description(&self) -> &str {
        match *self {
            BabyProverErr::Unknown => "Unknown error",
            BabyProverErr::InvalidAmountEncoding => "transfer amount is malformed or too large",
            BabyProverErr::InvalidFeeEncoding => "transfer fee is malformed or too large",
            BabyProverErr::InvalidSender => "sender account is unknown",
            BabyProverErr::InvalidRecipient => "recipient account is unknown",
            BabyProverErr::InvalidTransaction(_) => "invalid tx data",
            BabyProverErr::IoError(_) => "encountered an I/O error",
        }
    }
}

impl fmt::Display for BabyProverErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if let &BabyProverErr::IoError(ref e) = self {
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
    inputs: [Fr; 3],
    block_number: Fr,
    total_fees: Fr,
    public_data: Vec<u8>,
}

fn read_parameters(file_name: &str) -> Result<BabyParameters, BabyProverErr> {
    use std::fs::File;
    use std::io::{BufReader};

    let f_r = File::open("transfer_pk.key");   
    if f_r.is_err() {
        return Err(BabyProverErr::IoError(f_r.err().unwrap()));
    }
    let mut r = BufReader::new(f_r.unwrap());
    let circuit_params = BabyParameters::read(& mut r, true);

    if circuit_params.is_err() {
        return Err(BabyProverErr::IoError(circuit_params.err().unwrap()));
    }

    Ok(circuit_params.unwrap())
}

impl BabyProver {

    pub fn create(initial_state: &PlasmaState) -> Result<BabyProver, BabyProverErr> {
        println!("Reading proving key, may take a while");

        let transfer_circuit_params = read_parameters("transfer_pk.key");
        if transfer_circuit_params.is_err() {
            return Err(transfer_circuit_params.err().unwrap());
        }

        println!("Done reading transfer key");

        let deposit_circuit_params = read_parameters("deposit_pk.key");
        if deposit_circuit_params.is_err() {
            return Err(deposit_circuit_params.err().unwrap());
        }

        println!("Done reading deposit key");

        let exit_circuit_params = read_parameters("exit_pk.key");
        if exit_circuit_params.is_err() {
            return Err(exit_circuit_params.err().unwrap());
        }

        println!("Done reading exit key");

        println!("Copying states to balance tree");

        // TODO: replace with .clone() by moving PedersenHasher to static context
        let mut tree = AccountTree::new(params::BALANCE_TREE_DEPTH as u32);
        {
            let iter = initial_state.get_accounts().into_iter();

            for e in iter {
                let acc_number = e.0;
                let leaf_copy = Account::from(e.1.clone());
                tree.insert(acc_number, leaf_copy);
            }
        }

        let root = tree.root_hash();

        println!("Root hash is {}", root);

        let supplied_root = initial_state.root_hash();

        if root != supplied_root {
            return Err(BabyProverErr::Unknown);
        }

        let jubjub_params = AltJubjubBn256::new();

        Ok(Self{
            transfer_batch_size: TRANSFER_BATCH_SIZE,
            deposit_batch_size: DEPOSIT_BATCH_SIZE,
            exit_batch_size: EXIT_BATCH_SIZE,
            block_number: 1,
            accounts_tree: tree,
            transfer_parameters: transfer_circuit_params.unwrap(),
            deposit_parameters: deposit_circuit_params.unwrap(),
            exit_parameters: exit_circuit_params.unwrap(),
            jubjub_params: jubjub_params
        })
    }
}

type Err = BabyProverErr;

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

        let new_root = serialize_fe_for_ethereum(proof.inputs[1]);

        let total_fees = serialize_fe_for_ethereum(proof.total_fees);

        let block_number = serialize_fe_for_ethereum(proof.block_number);

        let public_data = proof.public_data.clone();

        let p = EncodedProof{
            groth_proof: [a_x, a_y, b_x_0, b_x_1, b_y_0, b_y_1, c_x, c_y],
            block_number: block_number,
        };

        Ok(p)
    }

    pub fn encode_transfer_transactions(block: &TransferBlock) -> Result<Vec<u8>, Err> {
        let mut encoding: Vec<u8> = vec![];
        
        let transactions = &block.transactions;

        for tx in transactions {
            let tx = models::circuit::TransferTx::try_from(tx).map_err(|e| BabyProverErr::InvalidTransaction(e.to_string()))?;
            let tx_bits = tx.public_data_into_bits();
            let tx_encoding = be_bit_vector_into_bytes(&tx_bits);
            encoding.extend(tx_encoding.into_iter());
        }

        Ok(encoding)
    }

    pub fn encode_deposit_transactions(block: &DepositBlock) -> Result<Vec<u8>, Err> {
        return Ok(vec![]);
        // let mut encoding: Vec<u8> = vec![];
        
        // let transactions = &block.transactions;

        // for tx in transactions {
        //     let tx = models::circuit::DepositRequest::try_from(tx).map_err(|e| BabyProverErr::InvalidTransaction(e.to_string()))?;
        //     let tx_bits = tx.public_data_into_bits();
        //     let tx_encoding = be_bit_vector_into_bytes(&tx_bits);
        //     encoding.extend(tx_encoding.into_iter());
        // }

        // Ok(encoding)
    }

    // this method is different, it actually reads the state 
    pub fn encode_exit_transactions(block: &ExitBlock) -> Result<Vec<u8>, Err> {
        return Ok(vec![]);
        // let mut encoding: Vec<u8> = vec![];
        
        // let transactions = &block.transactions;

        // for tx in transactions {
        //     let tx = models::circuit::ExitRequest::try_from(tx).map_err(|e| BabyProverErr::InvalidTransaction(e.to_string()))?;
        //     let tx_bits = tx.public_data_into_bits();
        //     let tx_encoding = be_bit_vector_into_bytes(&tx_bits);
        //     encoding.extend(tx_encoding.into_iter());
        // }

        // Ok(encoding)
    }

    pub fn apply_and_prove(&mut self, block: Block) -> Result<FullBabyProof, Err> {
        match block {
            Block::Deposit(block) => {
                return self.apply_and_prove_deposit(&block);
            },
            Block::Exit(block) => {
                unimplemented!()
            },
            Block::Transfer(block) => {
                return self.apply_and_prove_transfer(&block);
            },
        }
    }

    // Apply transactions to the state while also making a witness for proof, then calculate proof
    pub fn apply_and_prove_transfer(&mut self, block: &TransferBlock) -> Result<FullBabyProof, Err> {
        let block_number = block.block_number;
        if block_number != self.block_number {
            return Err(BabyProverErr::Unknown);
        }
        let block_final_root = block.new_root_hash.clone();

        let public_data: Vec<u8> = BabyProver::encode_transfer_transactions(block).unwrap();

        let transactions = &block.transactions;
        let num_txes = transactions.len();

        if num_txes != self.transfer_batch_size {
            return Err(BabyProverErr::Unknown);
        }

        let mut witnesses: Vec<(Transaction<Engine>, TransactionWitness<Engine>)> = vec![];

        let mut total_fees = Fr::zero();

        let initial_root = self.accounts_tree.root_hash();

        for tx in transactions {
            let tx = models::circuit::TransferTx::try_from(tx).map_err(|e| BabyProverErr::InvalidTransaction(e.to_string()))?;
            let sender_leaf_number = field_element_to_u32(tx.from);
            let recipient_leaf_number = field_element_to_u32(tx.to);

            let tree = &mut self.accounts_tree;
            let items = tree.items.clone();

            let sender_leaf = items.get(&sender_leaf_number);
            let recipient_leaf = items.get(&recipient_leaf_number);

            if sender_leaf.is_none() || recipient_leaf.is_none() {
                return Err(BabyProverErr::InvalidSender);
            }
            
            // this is LE bits encoding of the transaction amount
            let mut amount_bits: Vec<bool> = BitIterator::new(tx.amount.into_repr()).collect();
            amount_bits.reverse();
            amount_bits.truncate(params::AMOUNT_EXPONENT_BIT_WIDTH + params::AMOUNT_MANTISSA_BIT_WIDTH);

            let parsed_transfer_amount = parse_float_to_u128(amount_bits, 
                params::AMOUNT_EXPONENT_BIT_WIDTH,
                params::AMOUNT_MANTISSA_BIT_WIDTH,
                10
            );

            // this is LE bits encoding of the transaction fee
            let mut fee_bits: Vec<bool>  = BitIterator::new(tx.fee.into_repr()).collect();
            fee_bits.reverse();
            fee_bits.truncate(params::FEE_EXPONENT_BIT_WIDTH + params::FEE_MANTISSA_BIT_WIDTH);

            let parsed_fee = parse_float_to_u128(fee_bits, 
                params::FEE_EXPONENT_BIT_WIDTH,
                params::FEE_MANTISSA_BIT_WIDTH,
                10
            );

            if parsed_transfer_amount.is_err() || parsed_fee.is_err() {
                return Err(BabyProverErr::InvalidAmountEncoding);
            }

            let transfer_amount_as_field_element = Fr::from_str(&parsed_transfer_amount.unwrap().to_string()).unwrap();
            let fee_as_field_element = Fr::from_str(&parsed_fee.unwrap().to_string()).unwrap();

            let path_from : Vec<Option<Fr>> = tree.merkle_path(sender_leaf_number).into_iter().map(|e| Some(e.0)).collect();
            let path_to: Vec<Option<Fr>> = tree.merkle_path(recipient_leaf_number).into_iter().map(|e| Some(e.0)).collect();

            let transaction = Transaction {
                from: Some(tx.from.clone()),
                to: Some(tx.to.clone()),
                amount: Some(tx.amount.clone()),
                fee: Some(tx.fee.clone()),
                nonce: Some(tx.nonce.clone()),
                good_until_block: Some(tx.good_until_block.clone()),
                signature: Some(tx.signature.clone())
            };

            let mut updated_sender_leaf = sender_leaf.unwrap().clone();
            let mut updated_recipient_leaf = recipient_leaf.unwrap().clone();

            updated_sender_leaf.balance.sub_assign(&transfer_amount_as_field_element);
            updated_sender_leaf.balance.sub_assign(&fee_as_field_element);

            updated_sender_leaf.nonce.add_assign(&Fr::one());

            updated_recipient_leaf.balance.add_assign(&transfer_amount_as_field_element);

            total_fees.add_assign(&fee_as_field_element);

            tree.insert(sender_leaf_number, updated_sender_leaf.clone());
            tree.insert(recipient_leaf_number, updated_recipient_leaf.clone());

            {
                let sender_leaf = sender_leaf.unwrap();

                let recipient_leaf = recipient_leaf.unwrap();

                let transaction_witness = TransactionWitness::<Engine>{
                    auth_path_from:     path_from,
                    leaf_from:          LeafWitness::<Engine>{
                                            balance:    Some(sender_leaf.balance),
                                            nonce:      Some(sender_leaf.nonce),
                                            pub_x:      Some(sender_leaf.pub_x),
                                            pub_y:      Some(sender_leaf.pub_y),
                                        },
                    auth_path_to:       path_to,
                    leaf_to:            LeafWitness::<Engine>{
                                            balance:    Some(recipient_leaf.balance),
                                            nonce:      Some(recipient_leaf.nonce),
                                            pub_x:      Some(recipient_leaf.pub_x),
                                            pub_y:      Some(recipient_leaf.pub_y),
                                        },                                        
                };

                let witness = (transaction.clone(), transaction_witness);

                witnesses.push(witness);
            }
        }

        let block_number = Fr::from_str(&block_number.to_string()).unwrap();

        let final_root = self.accounts_tree.root_hash();

        if initial_root == final_root {
            return Err(BabyProverErr::Unknown);
        }

        println!("Prover final root = {}, final root from state keeper = {}", final_root, block_final_root);

        if block_final_root != final_root {
            return Err(BabyProverErr::Unknown);
        }

        self.block_number += 1;

        let mut public_data_initial_bits = vec![];

        // these two are BE encodings because an iterator is BE. This is also an Ethereum standard behavior

        let block_number_bits: Vec<bool> = BitIterator::new(block_number.into_repr()).collect();
        for _ in 0..256-block_number_bits.len() {
            public_data_initial_bits.push(false);
        }
        public_data_initial_bits.extend(block_number_bits.into_iter());

        let total_fee_bits: Vec<bool> = BitIterator::new(total_fees.into_repr()).collect();
        for _ in 0..256-total_fee_bits.len() {
            public_data_initial_bits.push(false);
        }
        public_data_initial_bits.extend(total_fee_bits.into_iter());

        assert_eq!(public_data_initial_bits.len(), 512);

        let mut h = Sha256::new();

        let bytes_to_hash = be_bit_vector_into_bytes(&public_data_initial_bits);

        h.input(&bytes_to_hash);

        let mut hash_result = [0u8; 32];
        h.result(&mut hash_result[..]);

        {    
            let packed_transaction_data_bytes = public_data.clone();

            let mut next_round_hash_bytes = vec![];
            next_round_hash_bytes.extend(hash_result.iter());
            next_round_hash_bytes.extend(packed_transaction_data_bytes);

            let mut h = Sha256::new();

            h.input(&next_round_hash_bytes);

            h.result(&mut hash_result[..]);
        }

        // clip to fit into field element

        hash_result[0] &= 0x1f; // temporary solution

        let mut repr = Fr::zero().into_repr();
        repr.read_be(&hash_result[..]).expect("pack hash as field element");
        
        let public_data_commitment = Fr::from_repr(repr).unwrap();

        let instance = Transfer {
            params: &self.jubjub_params,
            number_of_transactions: num_txes,
            old_root: Some(initial_root),
            new_root: Some(final_root),
            public_data_commitment: Some(public_data_commitment),
            block_number: Some(block_number),
            total_fee: Some(total_fees),
            transactions: witnesses.clone(),
        };

        let mut rng = OsRng::new().unwrap();
        println!("Prover has started to work");
        let proof = create_random_proof(instance, &self.transfer_parameters, & mut rng);
        if proof.is_err() {
            return Err(BabyProverErr::Unknown);
        }

        let p = proof.unwrap();

        let pvk = prepare_verifying_key(&self.transfer_parameters.vk);

        let success = verify_proof(&pvk, &p.clone(), &[initial_root, final_root, public_data_commitment]).unwrap();
        
        if !success {
            return Err(BabyProverErr::Unknown);
        }
        println!("Proof generation is complete");

        let full_proof = FullBabyProof{
            proof: p,
            inputs: [initial_root, final_root, public_data_commitment],
            total_fees: total_fees,
            block_number: block_number,
            public_data: public_data,
        };

        Ok(full_proof)
    }

    // TODO: sort accounts!
    pub fn apply_and_prove_deposit(&mut self, block: &DepositBlock) -> Result<FullBabyProof, Err> {
        let block_number = block.block_number;
        if block_number != self.block_number {
            return Err(BabyProverErr::Unknown);
        }
        let block_final_root = block.new_root_hash.clone();

        let public_data: Vec<u8> = BabyProver::encode_deposit_transactions(block).unwrap();

        let transactions = &block.transactions;
        let num_txes = transactions.len();

        if num_txes != self.deposit_batch_size {
            return Err(BabyProverErr::Unknown);
        }

        let mut witnesses: Vec<(DepositRequest<Engine>, DepositWitness<Engine>)> = vec![];

        let initial_root = self.accounts_tree.root_hash();

        for tx in transactions {
            let tx = models::circuit::DepositRequest::try_from(tx).map_err(|e| BabyProverErr::InvalidTransaction(e.to_string()))?;
            
            let into_leaf_number = field_element_to_u32(tx.into);
            
            let tree = &mut self.accounts_tree;
            let items = tree.items.clone();

            let existing_leaf = items.get(&into_leaf_number);
            let mut leaf_is_empty = true;

            let (old_leaf, new_leaf) = if existing_leaf.is_none() {
                let mut new_leaf = Account::default();
                new_leaf.balance = tx.amount.clone();
                new_leaf.pub_x = tx.pub_x;
                new_leaf.pub_y = tx.pub_y;

                (Account::default(), new_leaf)
            } else {
                let old_leaf = existing_leaf.unwrap().clone();
                let mut new_leaf = old_leaf.clone();
                new_leaf.balance.add_assign(&tx.amount);
                leaf_is_empty = false;

                (old_leaf, new_leaf)
            };

            let path: Vec<Option<Fr>> = tree.merkle_path(into_leaf_number).into_iter().map(|e| Some(e.0)).collect();

            let public_key = edwards::Point::from_xy(new_leaf.pub_x.clone(), new_leaf.pub_y.clone(), &self.jubjub_params);

            if public_key.is_none() {
                return Err(BabyProverErr::Unknown);
            }

            let request = DepositRequest {
                into: Fr::from_str(&into_leaf_number.to_string()),
                amount: Some(tx.amount),
                public_key: public_key
            };

            tree.insert(into_leaf_number, new_leaf.clone());

            {
                let deposit_witness = DepositWitness::<Engine>{
                    auth_path:     path,
                    leaf:          LeafWitness::<Engine>{
                                        balance:    Some(old_leaf.balance),
                                        nonce:      Some(old_leaf.nonce),
                                        pub_x:      Some(old_leaf.pub_x),
                                        pub_y:      Some(old_leaf.pub_y),
                                    },          

                    leaf_is_empty: Some(leaf_is_empty),
                    new_pub_x: Some(new_leaf.pub_x),
                    new_pub_y: Some(new_leaf.pub_y),                     
                };

                let witness = (request.clone(), deposit_witness);

                witnesses.push(witness);
            }
        }

        let block_number = Fr::from_str(&block_number.to_string()).unwrap();

        let final_root = self.accounts_tree.root_hash();

        if initial_root == final_root {
            return Err(BabyProverErr::Unknown);
        }

        println!("Prover final root = {}, final root from state keeper = {}", final_root, block_final_root);

        if block_final_root != final_root {
            return Err(BabyProverErr::Unknown);
        }

        self.block_number += 1;

        let mut public_data_initial_bits = vec![];

        // these two are BE encodings because an iterator is BE. This is also an Ethereum standard behavior

        let block_number_bits: Vec<bool> = BitIterator::new(block_number.into_repr()).collect();
        for _ in 0..256-block_number_bits.len() {
            public_data_initial_bits.push(false);
        }
        public_data_initial_bits.extend(block_number_bits.into_iter());

        assert_eq!(public_data_initial_bits.len(), 256);

        let mut h = Sha256::new();

        let bytes_to_hash = be_bit_vector_into_bytes(&public_data_initial_bits);

        h.input(&bytes_to_hash);

        let mut hash_result = [0u8; 32];
        h.result(&mut hash_result[..]);

        {    
            let packed_transaction_data_bytes = public_data.clone();

            let mut next_round_hash_bytes = vec![];
            next_round_hash_bytes.extend(hash_result.iter());
            next_round_hash_bytes.extend(packed_transaction_data_bytes);

            let mut h = Sha256::new();

            h.input(&next_round_hash_bytes);

            h.result(&mut hash_result[..]);
        }

        // clip to fit into field element

        hash_result[0] &= 0x1f; // temporary solution

        let mut repr = Fr::zero().into_repr();
        repr.read_be(&hash_result[..]).expect("pack hash as field element");
        
        let public_data_commitment = Fr::from_repr(repr).unwrap();

        let instance = Deposit {
            params: &self.jubjub_params,
            number_of_deposits: num_txes,
            old_root: Some(initial_root),
            new_root: Some(final_root),
            public_data_commitment: Some(public_data_commitment),
            block_number: Some(block_number),
            requests: witnesses.clone(),
        };

        let mut rng = OsRng::new().unwrap();
        println!("Prover has started to work");
        let proof = create_random_proof(instance, &self.deposit_parameters, & mut rng);
        if proof.is_err() {
            return Err(BabyProverErr::Unknown);
        }

        let p = proof.unwrap();

        let pvk = prepare_verifying_key(&self.deposit_parameters.vk);

        let success = verify_proof(&pvk, &p.clone(), &[initial_root, final_root, public_data_commitment]).unwrap();
        
        if !success {
            return Err(BabyProverErr::Unknown);
        }
        println!("Proof generation is complete");

        let full_proof = FullBabyProof{
            proof: p,
            inputs: [initial_root, final_root, public_data_commitment],
            total_fees: Fr::zero(),
            block_number: block_number,
            public_data: public_data, // it's effectively zero
        };

        Ok(full_proof)
    }

    // TODO: sort accounts!
    pub fn apply_and_prove_exit(&mut self, block: &ExitBlock) -> Result<FullBabyProof, Err> {
        let block_number = block.block_number;
        if block_number != self.block_number {
            return Err(BabyProverErr::Unknown);
        }
        let block_final_root = block.new_root_hash.clone();

        let transactions = &block.transactions;
        let num_txes = transactions.len();

        if num_txes != self.deposit_batch_size {
            return Err(BabyProverErr::Unknown);
        }

        let mut witnesses: Vec<(ExitRequest<Engine>, ExitWitness<Engine>)> = vec![];

        let initial_root = self.accounts_tree.root_hash();

        // we also need to create public data cause we need info from state
        let mut public_data: Vec<u8> = vec![];

        for tx in transactions {
            let tx = models::circuit::ExitRequest::try_from(tx).map_err(|e| BabyProverErr::InvalidTransaction(e.to_string()))?;
            
            let from_leaf_number = field_element_to_u32(tx.from);
            
            let tree = &mut self.accounts_tree;
            let items = tree.items.clone();

            let existing_leaf = items.get(&from_leaf_number);

            if existing_leaf.is_none() {
                return Err(BabyProverErr::Unknown);
            }

            let old_leaf = existing_leaf.unwrap();

            let new_leaf = Account::default();

            let path: Vec<Option<Fr>> = tree.merkle_path(from_leaf_number).into_iter().map(|e| Some(e.0)).collect();

            let request = ExitRequest {
                from: Fr::from_str(&from_leaf_number.to_string()),
                amount: Some(old_leaf.balance),
            };

            // we have the leaf info, so add it to the public data
            let tx_bits = request.public_data_into_bits();
            let tx_encoding = be_bit_vector_into_bytes(&tx_bits);
            public_data.extend(tx_encoding.into_iter());

            tree.insert(from_leaf_number, new_leaf.clone());

            {
                let deposit_witness = ExitWitness::<Engine>{
                    auth_path:     path,
                    leaf:          LeafWitness::<Engine>{
                                        balance:    Some(old_leaf.balance),
                                        nonce:      Some(old_leaf.nonce),
                                        pub_x:      Some(old_leaf.pub_x),
                                        pub_y:      Some(old_leaf.pub_y),
                                    },                     
                };

                let witness = (request.clone(), deposit_witness);

                witnesses.push(witness);
            }
        }

        let block_number = Fr::from_str(&block_number.to_string()).unwrap();

        let final_root = self.accounts_tree.root_hash();

        if initial_root == final_root {
            return Err(BabyProverErr::Unknown);
        }

        println!("Prover final root = {}, final root from state keeper = {}", final_root, block_final_root);

        if block_final_root != final_root {
            return Err(BabyProverErr::Unknown);
        }

        self.block_number += 1;

        let mut public_data_initial_bits = vec![];

        // these two are BE encodings because an iterator is BE. This is also an Ethereum standard behavior

        let block_number_bits: Vec<bool> = BitIterator::new(block_number.into_repr()).collect();
        for _ in 0..256-block_number_bits.len() {
            public_data_initial_bits.push(false);
        }
        public_data_initial_bits.extend(block_number_bits.into_iter());

        assert_eq!(public_data_initial_bits.len(), 256);

        let mut h = Sha256::new();

        let bytes_to_hash = be_bit_vector_into_bytes(&public_data_initial_bits);

        h.input(&bytes_to_hash);

        let mut hash_result = [0u8; 32];
        h.result(&mut hash_result[..]);

        {    
            let packed_transaction_data_bytes = public_data.clone();
        
            let mut next_round_hash_bytes = vec![];
            next_round_hash_bytes.extend(hash_result.iter());
            next_round_hash_bytes.extend(packed_transaction_data_bytes);

            let mut h = Sha256::new();

            h.input(&next_round_hash_bytes);

            h.result(&mut hash_result[..]);
        }

        // clip to fit into field element

        hash_result[0] &= 0x1f; // temporary solution

        let mut repr = Fr::zero().into_repr();
        repr.read_be(&hash_result[..]).expect("pack hash as field element");
        
        let public_data_commitment = Fr::from_repr(repr).unwrap();

        let empty_leaf_witness = LeafWitness::<Engine> {
            balance:    Some(Fr::zero()),
            nonce:      Some(Fr::zero()),
            pub_x:      Some(Fr::zero()),
            pub_y:      Some(Fr::zero()),
        };

        let instance = Exit {
            params: &self.jubjub_params,
            number_of_exits: num_txes,
            old_root: Some(initial_root),
            new_root: Some(final_root),
            public_data_commitment: Some(public_data_commitment),
            empty_leaf_witness: empty_leaf_witness,
            block_number: Some(block_number),
            requests: witnesses.clone(),
        };

        let mut rng = OsRng::new().unwrap();
        println!("Prover has started to work");
        let proof = create_random_proof(instance, &self.exit_parameters, & mut rng);
        if proof.is_err() {
            return Err(BabyProverErr::Unknown);
        }

        let p = proof.unwrap();

        let pvk = prepare_verifying_key(&self.exit_parameters.vk);

        let success = verify_proof(&pvk, &p.clone(), &[initial_root, final_root, public_data_commitment]).unwrap();
        
        if !success {
            return Err(BabyProverErr::Unknown);
        }
        println!("Proof generation is complete");

        let full_proof = FullBabyProof{
            proof: p,
            inputs: [initial_root, final_root, public_data_commitment],
            total_fees: Fr::zero(),
            block_number: block_number,
            public_data: public_data,
        };

        Ok(full_proof)
    }

    pub fn run(
            &mut self,
            rx_for_blocks: mpsc::Receiver<Block>, 
            tx_for_proofs: mpsc::Sender<BlockProof>
        ) 
    {
        for block in rx_for_blocks {
            println!("Got request for proof");

            let proof = self.apply_and_prove(block).unwrap();
            let full_proof = BabyProver::encode_proof(&proof).unwrap();
            let accounts = vec![]; // TODO: pass updated states of affected accounts
            tx_for_proofs.send(BlockProof(full_proof, accounts));

            // match block {
            //     Block::Transfer(block) => {
            //         let proof = self.apply_and_prove(&block).unwrap();
            //         let full_proof = BabyProver::encode_proof(&proof).unwrap();
            //         let accounts = vec![]; // TODO: pass updated states of affected accounts
            //         tx_for_proofs.send(BlockProof(full_proof, accounts));
            //     },
            //     _ => unimplemented!(),
            // }
        }        
    }
    
}