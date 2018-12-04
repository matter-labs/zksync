use std::error::Error;
use ff::{Field, PrimeField, PrimeFieldRepr, BitIterator};
use super::plasma_state::{State, Block};
use super::prover::{Prover};
use std::fmt;
use rand::{OsRng, Rng};

use super::super::circuit::plasma_constants;
use super::super::balance_tree;
use super::super::circuit::utils::be_bit_vector_into_bytes;
use super::super::circuit::baby_plasma::{Update, Transaction, TransactionWitness};

use sapling_crypto::circuit::float_point::parse_float_to_u128;
use sapling_crypto::alt_babyjubjub::{AltJubjubBn256};
use pairing::CurveAffine;

use pairing::bn256::{Bn256, Fr};
use bellman::groth16::{Proof, Parameters, create_random_proof, verify_proof, prepare_verifying_key};

use web3::types::{U256, Bytes};

use crypto::sha2::Sha256;
use crypto::digest::Digest;

#[derive(Debug)]
pub enum BabyProverErr {
    Unknown,
    InvalidAmountEncoding,
    InvalidFeeEncoding,
    InvalidSender,
    InvalidRecipient,
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

pub struct BabyProver {
    batch_size: usize,
    accounts_tree: balance_tree::BabyBalanceTree,
    parameters: BabyParameters,
    jubjub_params: AltJubjubBn256,
}

#[derive(Debug, Clone)]
pub struct EthereumProof {
    groth_proof: [U256; 8],
    new_root: U256,
    block_number: U256,
    total_fees: U256,
    public_data: Bytes,
}

#[derive(Debug)]
pub struct FullBabyProof {
    proof: BabyProof,
    inputs: [Fr; 3],
    block_number: Fr,
    total_fees: Fr,
    public_data: Vec<u8>,
}

type BabyProof = Proof<Bn256>;
type BabyParameters = Parameters<Bn256>;

fn field_element_to_u32<P: PrimeField>(fr: P) -> u32 {
    let mut iterator: Vec<bool> = BitIterator::new(fr.into_repr()).collect();
    iterator.reverse();
    iterator.truncate(32);
    let mut res = 0u32;
    let mut base = 1u32;
    for bit in iterator {
        if bit {
            res += base;
        }
        base = base << 1;
    }

    res
}

const TX_BATCH_SIZE: usize = 8;

impl BabyProver {
    pub fn create<'a>(initial_state: &'a State<'a, Bn256>) ->
        Result<BabyProver, BabyProverErr>
    {
        use std::fs::File;
        use std::io::{BufReader};

        println!("Reading proving key, may take a while");

        let f_r = File::open("pk.key");
        if f_r.is_err() {
            return Err(BabyProverErr::IoError(f_r.err().unwrap()));
        }
        let mut r = BufReader::new(f_r.unwrap());
        let circuit_params = BabyParameters::read(& mut r, true);

        if circuit_params.is_err() {
            return Err(BabyProverErr::IoError(circuit_params.err().unwrap()));
        }

        println!("Copying states to balance tree");

        let mut tree = balance_tree::BabyBalanceTree::new(*plasma_constants::BALANCE_TREE_DEPTH as u32);
        {
            let iter = initial_state.get_accounts().into_iter();

            for e in iter {
                let acc_number = e.0;
                let leaf_copy = e.1.clone();
                tree.insert(acc_number, leaf_copy);
            }
        }

        let root = tree.root_hash();

        println!("Root hash is {}", root);

        let supplied_root = initial_state.root_hash();

        if root != supplied_root {
            return Err(BabyProverErr::Unknown);
        }

        let params = circuit_params.unwrap();

        let jubjub_params = AltJubjubBn256::new();

        Ok(Self{
            batch_size: TX_BATCH_SIZE,
            accounts_tree: tree,
            parameters: params,
            jubjub_params: jubjub_params
        })
    }
}

impl Prover<Bn256> for BabyProver {

    type Err = BabyProverErr;
    type Proof = FullBabyProof;
    type EncodedProof = EthereumProof;

    // Outputs
    // - 8 uint256 for encoding of the field elements
    // - one uint256 for new root hash
    // - uint256 block number
    // - uint256 total fees
    // - Bytes public data
    //
    // Old root is available to take from the storage of the smart-contract
    fn encode_proof(proof: &Self::Proof) -> Result<Self::EncodedProof, Self::Err> {

        // proof     
        // pub a: E::G1Affine,
        // pub b: E::G2Affine,
        // pub c: E::G1Affine

        let a_uncompressed = proof.proof.a.into_uncompressed();
        let b_uncompressed = proof.proof.b.into_uncompressed();
        let c_uncompressed = proof.proof.c.into_uncompressed();

        let a_x = U256::from_big_endian(& a_uncompressed.as_ref()[0..32]);
        let a_y = U256::from_big_endian(& a_uncompressed.as_ref()[32..64]);

        let b_x_0 = U256::from_big_endian(& b_uncompressed.as_ref()[0..32]);
        let b_x_1 = U256::from_big_endian(& b_uncompressed.as_ref()[32..64]);
        let b_y_0 = U256::from_big_endian(& b_uncompressed.as_ref()[64..92]);
        let b_y_1 = U256::from_big_endian(& b_uncompressed.as_ref()[92..128]);


        let c_x = U256::from_big_endian(& c_uncompressed.as_ref()[0..32]);
        let c_y = U256::from_big_endian(& c_uncompressed.as_ref()[32..64]);

        let mut root_hash_be_bytes = [0u8; 32];
        proof.inputs[1].into_repr().write_be(& mut root_hash_be_bytes[..]).expect("get new root BE bytes");
        let new_root = U256::from_big_endian(&root_hash_be_bytes[..]);

        let mut block_number_be_bytes = [0u8; 32];
        proof.block_number.into_repr().write_be(& mut block_number_be_bytes[..]).expect("get block number BE bytes");
        let block_number = U256::from_big_endian(&block_number_be_bytes[..]);

        let mut total_fees_be_bytes = [0u8; 32];
        proof.total_fees.into_repr().write_be(& mut total_fees_be_bytes[..]).expect("get total fees BE bytes");
        let total_fees = U256::from_big_endian(&total_fees_be_bytes[..]);

        let public_data = Bytes::from(proof.public_data.clone());

        let p = EthereumProof{
            groth_proof: [a_x, a_y, b_x_0, b_x_1, b_y_0, b_y_1, c_x, c_y],
            new_root: new_root,
            block_number: block_number,
            total_fees: total_fees,
            public_data: public_data,
        };

        Ok(p)
    }


    // Takes public data from transactions for further commitment to Ethereum
    fn encode_transactions(block: &Block<Bn256>) -> Result<Vec<u8>, Self::Err> {
        let mut encoding : Vec<u8> = vec![];
        let transactions = &block.transactions;

        for tx in transactions {
            let tx_bits = tx.public_data_into_bits();
            let tx_encoding = be_bit_vector_into_bytes(&tx_bits);
            encoding.extend(tx_encoding.into_iter());
        }
        Ok(encoding)
    }

    // Apply transactions to the state while also making a witness for proof, then calculate proof
    fn apply_and_prove(&mut self, block: &Block<Bn256>) -> Result<Self::Proof, Self::Err> {
        let block_number = block.block_number;
        let public_data: Vec<u8> = BabyProver::encode_transactions(block).unwrap();

        let transactions = &block.transactions;
        let num_txes = transactions.len();

        if num_txes != self.batch_size {
            return Err(BabyProverErr::Unknown);
        }

        let mut witnesses: Vec<Option<(Transaction<Bn256>, TransactionWitness<Bn256>)>> = vec![];

        let mut total_fees = Fr::zero();

        let initial_root = self.accounts_tree.root_hash();

        for tx in transactions {
            let sender_leaf_number = field_element_to_u32(tx.from);
            let recipient_leaf_number = field_element_to_u32(tx.to);

            let mut tree = & mut self.accounts_tree;
            let mut items = tree.items.clone();

            let sender_leaf = items.get(&sender_leaf_number);
            let recipient_leaf = items.get(&recipient_leaf_number);

            if sender_leaf.is_none() || recipient_leaf.is_none() {
                return Err(BabyProverErr::InvalidSender);
            }
            
            let parsed_transfer_amount = parse_float_to_u128(BitIterator::new(tx.amount.into_repr()).collect(), 
                *plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH,
                *plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH,
                10
            );

            let parsed_fee = parse_float_to_u128(BitIterator::new(tx.fee.into_repr()).collect(), 
                *plasma_constants::FEE_EXPONENT_BIT_WIDTH,
                *plasma_constants::FEE_MANTISSA_BIT_WIDTH,
                10
            );

            if parsed_transfer_amount.is_err() || parsed_fee.is_err() {
                return Err(BabyProverErr::InvalidAmountEncoding);
            }

            let transfer_amount_as_field_element = Fr::from_str(&parsed_transfer_amount.unwrap().to_string()).unwrap();
            let fee_as_field_element = Fr::from_str(&parsed_fee.unwrap().to_string()).unwrap();

            let path_from : Vec<Option<(Fr, bool)>> = tree.merkle_path(sender_leaf_number).into_iter().map(|e| Some(e)).collect();
            let path_to: Vec<Option<(Fr, bool)>> = tree.merkle_path(recipient_leaf_number).into_iter().map(|e| Some(e)).collect();

            let mut transaction : Transaction<Bn256> = Transaction {
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

                let transaction_witness = TransactionWitness {
                    auth_path_from: path_from,
                    balance_from: Some(sender_leaf.balance),
                    nonce_from: Some(sender_leaf.nonce),
                    pub_x_from: Some(sender_leaf.pub_x),
                    pub_y_from: Some(sender_leaf.pub_y),
                    auth_path_to: path_to,
                    balance_to: Some(recipient_leaf.balance),
                    nonce_to: Some(recipient_leaf.nonce),
                    pub_x_to: Some(recipient_leaf.pub_x),
                    pub_y_to: Some(recipient_leaf.pub_y)
                };

                let witness = (transaction.clone(), transaction_witness);

                witnesses.push(Some(witness));
            }
        }

        let block_number = Fr::from_str(&block_number.to_string()).unwrap();

        let final_root = self.accounts_tree.root_hash();

        if initial_root == final_root {
            return Err(BabyProverErr::Unknown);
        }

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

        let instance = Update {
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

        let proof = create_random_proof(instance, &self.parameters, & mut rng);
        if proof.is_err() {
            return Err(BabyProverErr::Unknown);
        }

        let p = proof.unwrap();

        let pvk = prepare_verifying_key(&self.parameters.vk);

        let success = verify_proof(&pvk, &p.clone(), &[initial_root, final_root, public_data_commitment]).unwrap();
        
        if !success {
            return Err(BabyProverErr::Unknown);
        }

        let full_proof = FullBabyProof{
            proof: p,
            inputs: [initial_root, final_root, public_data_commitment],
            total_fees: total_fees,
            block_number: block_number,
            public_data: public_data,
        };

        Ok(full_proof)
    }
    
}