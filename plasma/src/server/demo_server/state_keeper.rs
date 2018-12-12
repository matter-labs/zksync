use super::super::plasma_state::{State, Account, Tx};
use super::super::super::balance_tree::BabyBalanceTree;
use super::super::super::primitives::{field_element_to_u32, field_element_to_u128};
use pairing::bn256::{Bn256, Fr};
use sapling_crypto::jubjub::{JubjubEngine, edwards, Unknown, FixedGenerators};
use sapling_crypto::circuit::float_point::{convert_to_float, parse_float_to_u128};
use sapling_crypto::alt_babyjubjub::{AltJubjubBn256};
use super::super::super::circuit::plasma_constants;
use super::super::super::circuit::transfer::transaction::{TransactionSignature};
use super::super::super::circuit::utils::{le_bit_vector_into_field_element};
use std::sync::mpsc;
use std::{thread, time};
use std::collections::HashMap;
use ff::{Field, PrimeField, PrimeFieldRepr, BitIterator};
use rand::{OsRng, Rng};
use sapling_crypto::eddsa::{PrivateKey, PublicKey};
use super::super::plasma_state::{Block};

#[derive(Debug, Clone)]
pub struct TxInfo{
    pub from: u32,
    pub to: u32,
    pub amount: u128,
    pub fee: u128,
    pub nonce: u32,
    pub good_until_block: u32
}

/// Coordinator of tx processing and generation of proofs
pub struct PlasmaStateKeeper {

    /// Accounts stored in a sparse Merkle tree
    pub balance_tree: BabyBalanceTree,
    // balance_tree: ParallelBalanceTree,

    /// Current block number
    pub block_number: u32,

    /// channel to receive signed and verified transactions to apply
    pub transactions_channel: mpsc::Receiver<(TxInfo, mpsc::Sender<bool>)>,

    // outgoing channel
    pub batch_channel: mpsc::Sender<Block<Bn256>>,

    // Batch size
    pub batch_size : usize,

    // Accumulated transactions
    pub current_batch: Vec<Tx<Bn256>>,

    // Keep private keys in memory
    pub private_keys: HashMap<u32, PrivateKey<Bn256>>
}

impl State<Bn256> for PlasmaStateKeeper {

    fn get_accounts(&self) -> Vec<(u32, Account<Bn256>)> {
        self.balance_tree.items.iter().map(|a| (*a.0 as u32, a.1.clone()) ).collect()
    }

    fn block_number(&self) -> u32 {
        self.block_number
    }

    fn root_hash (&self) -> Fr {
        self.balance_tree.root_hash().clone()
    }
}

impl PlasmaStateKeeper{

    pub fn run(& mut self) {
        loop {
            let message = self.transactions_channel.try_recv();
            if message.is_err() {
                thread::sleep(time::Duration::from_millis(10));
                continue;
            }
            let (tx, return_channel) = message.unwrap();
            println!("Got transaction!");
            self.apply_transaction(tx, return_channel);
        }
    }

    fn apply_transaction(& mut self, transaction: TxInfo, return_channel: mpsc::Sender<bool>) {
        {
            if transaction.good_until_block < self.block_number() {
                return_channel.send(true);
                return;
            }
        }

        let mut new_sender_leaf = None;
        let mut new_recipient_leaf = None;

        let from = transaction.from;
        let to = transaction.to;

        {
            let block_number = self.block_number;

            let tree = & mut self.balance_tree;

            // let mut items = tree.items;

            let current_sender_state = tree.items.get(&from);
            if current_sender_state.is_none() {
                return_channel.send(false);
                return;
            }

            let mut current_sender_state = current_sender_state.unwrap().clone();

            let current_balance = current_sender_state.balance;
            let current_nonce = current_sender_state.nonce;

            let balance_as_u128 = field_element_to_u128(current_balance);
            let nonce_as_u32 = field_element_to_u32(current_nonce);

            // if transaction.nonce != nonce_as_u32 {
            //     return_channel.send(false);
            //     return;
            // }

            if balance_as_u128 < transaction.amount {
                return_channel.send(false);
                return;
            }

            let encoded_amount_bits = convert_to_float(transaction.amount,
                *plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH, 
                *plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH, 
                10
            );

            if encoded_amount_bits.is_err() {
                return_channel.send(false);
                return;
            }
            
            // encoded bit are big endian by default!
            let encoded_amount_bits_unwrapped = encoded_amount_bits.unwrap();

            let encoded_amount: Fr = le_bit_vector_into_field_element(&encoded_amount_bits_unwrapped);

            // encoded fee is zero for now
            let encoded_fee = Fr::zero();

            // Will make a well-formed Tx by convering to field elements and making a signature

            let empty_point: edwards::Point<Bn256, Unknown> = edwards::Point::zero();

            let empty_signature = TransactionSignature{
                r: empty_point,
                s: Fr::zero()
            };

            let mut tx = Tx{
                from:               Fr::from_str(&from.to_string()).unwrap(),
                to:                 Fr::from_str(&to.to_string()).unwrap(),
                amount:             encoded_amount,
                fee:                encoded_fee,
                nonce:              current_nonce,
                good_until_block:   Fr::from_str(&block_number.to_string()).unwrap(),
                signature:          empty_signature,
            };

            let params = &AltJubjubBn256::new();
            let p_g = FixedGenerators::SpendingKeyGenerator;
            let mut rng = OsRng::new().unwrap();

            let sk = self.private_keys.get(& from).unwrap();

            tx.sign(sk, p_g, &params, & mut rng);

            let current_recipient_state = tree.items.get(&to);
            if current_recipient_state.is_none() {
                return_channel.send(false);
                return;
            }

            let mut current_recipient_state = current_recipient_state.unwrap().clone();

            let transfer_amount_as_field_element = Fr::from_str(&transaction.amount.to_string()).unwrap();

            // println!("Old sender account state:");
            // println!("Balance = {}", current_sender_state.balance);
            // println!("Nonce = {}", current_sender_state.nonce);
            // println!("Old recipient account state:");
            // println!("Balance = {}", current_recipient_state.balance);
            // println!("Nonce = {}", current_recipient_state.nonce);
            // println!("transfer_amount_as_field_element = {}", transfer_amount_as_field_element);
            // subtract from sender's balance
            current_sender_state.balance.sub_assign(&transfer_amount_as_field_element);
            // bump nonce
            current_sender_state.nonce.add_assign(&Fr::one());

            // add amount to recipient
            current_recipient_state.balance.add_assign(&transfer_amount_as_field_element);

            self.current_batch.push(tx);

            new_sender_leaf = Some(current_sender_state.clone());
            new_recipient_leaf = Some(current_recipient_state.clone());

        }

        self.balance_tree.insert(from, new_sender_leaf.unwrap());
        self.balance_tree.insert(to, new_recipient_leaf.unwrap());

        println!("Accepted transaction");

        let new_root = self.root_hash();

        println!("Intermediate root after the transaction application = {}", new_root);

        if self.current_batch.len() == self.batch_size {
            {
                let batch = &self.current_batch;
                let new_root = self.root_hash();
                let block: Block<Bn256> = Block {
                    block_number:   self.block_number,
                    transactions:   batch.to_vec(),
                    new_root_hash:  new_root,
                };
                self.batch_channel.send(block);
            }
            self.current_batch = Vec::with_capacity(self.batch_size);
            self.block_number += 1;
        }
        return_channel.send(true);
    }

}
