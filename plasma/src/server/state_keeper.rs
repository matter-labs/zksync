use pairing::bn256::{Bn256, Fr};
use sapling_crypto::jubjub::{edwards, Unknown, FixedGenerators};
use sapling_crypto::circuit::float_point::{convert_to_float};
use sapling_crypto::alt_babyjubjub::{AltJubjubBn256};

use crate::models::params;
use crate::models::state::{State};
use crate::primitives::{field_element_to_u32, field_element_to_u128};
use crate::circuit::utils::{le_bit_vector_into_field_element};
use std::sync::mpsc;
use std::{thread, time};
use std::collections::HashMap;
use ff::{Field, PrimeField};
use rand::{OsRng};
use sapling_crypto::eddsa::{PrivateKey};

use crate::models::baby_models::{Block, Account, Tx, AccountTree, TransactionSignature};

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
    pub balance_tree: AccountTree,

    /// Current block number
    pub block_number: u32,

    /// channel to receive signed and verified transactions to apply
    pub transactions_channel: mpsc::Receiver<(TxInfo, mpsc::Sender<bool>)>,

    // outgoing channel
    pub batch_channel: mpsc::Sender<Block>,

    // Batch size
    pub batch_size: usize,

    // Accumulated transactions
    pub current_batch: Vec<Tx>,

    // Keep private keys in memory
    pub private_keys: HashMap<u32, PrivateKey<Bn256>>
}

impl State<Bn256> for PlasmaStateKeeper {

    fn get_accounts(&self) -> Vec<(u32, Account)> {
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
            let r = self.handle_tx_request(&tx);
            return_channel.send(r.is_ok());
        }
    }

    fn empty_signature() -> TransactionSignature {
        let empty_point: edwards::Point<Bn256, Unknown> = edwards::Point::zero();
        TransactionSignature{
            r: empty_point,
            s: Fr::zero()
        }
    }

    // TODO: use nonce and good_until from transaction!
    fn pack_tx(transaction: &TxInfo, nonce: Fr, good_until: u32) -> Result<Tx, ()> {

        let encoded_amount_bits = convert_to_float(
            transaction.amount,
            params::AMOUNT_EXPONENT_BIT_WIDTH, 
            params::AMOUNT_MANTISSA_BIT_WIDTH, 
            10
        ).map_err(|_| ())?;
        let encoded_amount: Fr = le_bit_vector_into_field_element(&encoded_amount_bits);

        // encoded fee is zero for now
        let encoded_fee = Fr::zero();

        // Will make a well-formed Tx by convering to field elements and making a signature
        let tx = Tx {
            from:               Fr::from_str(&transaction.from.to_string()).unwrap(),
            to:                 Fr::from_str(&transaction.to.to_string()).unwrap(),
            amount:             encoded_amount,
            fee:                encoded_fee,
            nonce:              nonce,
            good_until_block:   Fr::from_str(&transaction.good_until_block.to_string()).unwrap(),
            signature:          Self::empty_signature(),
        };

        Ok(tx)
    }

    fn sign_tx(tx: &mut Tx, sk: &PrivateKey<Bn256>) {
        // TODO: move static params to constructor!
        let params = &AltJubjubBn256::new();
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let mut rng = OsRng::new().unwrap();
        tx.sign(sk, p_g, &params, &mut rng);
    }

    fn handle_tx_request(&mut self, transaction: &TxInfo) -> Result<(), ()> {
    
        // verify correctness

        let mut from = self.balance_tree.items.get(&transaction.from).ok_or(())?.clone();
        if field_element_to_u128(from.balance) < transaction.amount { return Err(()); }
        // TODO: check nonce: assert field_element_to_u32(from.nonce) == transaction.nonce

        // sign tx (for demo only; TODO: remove this)

        let mut tx = Self::pack_tx(&transaction, from.nonce, self.block_number)?;
        let sk = self.private_keys.get(&transaction.from).unwrap();
        Self::sign_tx(&mut tx, sk);

        // update state

        let mut to = self.balance_tree.items.get(&transaction.to).ok_or(())?.clone();
        let amount = Fr::from_str(&transaction.amount.to_string()).unwrap();
        from.balance.sub_assign(&amount);
        // TODO: subtract fee
        from.nonce.add_assign(&Fr::one());  // from.nonce++
        to.balance.add_assign(&amount);     // to.balance += amount
        self.balance_tree.insert(transaction.from, from);
        self.balance_tree.insert(transaction.to, to);

        // push for processing

        self.current_batch.push(tx);
        if self.current_batch.len() == self.batch_size {
            self.process_batch()
        }

        Ok(())
    }

    fn process_batch(&mut self) {
        
        let batch = &self.current_batch;
        let new_root = self.root_hash();
        let block = Block {
            block_number:   self.block_number,
            transactions:   batch.to_vec(),
            new_root_hash:  new_root,
        };
        self.batch_channel.send(block);

        self.current_batch = Vec::with_capacity(self.batch_size);
        self.block_number += 1;
    }

}
