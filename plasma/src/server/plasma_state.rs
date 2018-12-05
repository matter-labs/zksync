use std::collections::{hash_map, HashMap};

use sapling_crypto::alt_babyjubjub::{JubjubEngine};

use ff::{PrimeField, PrimeFieldRepr, BitIterator};

use super::super::circuit::plasma_constants;
use super::super::balance_tree;
use super::super::circuit::baby_plasma::TransactionSignature;

pub type Account<E> = balance_tree::Leaf<E>;

pub trait State<E: JubjubEngine> {  
    fn get_accounts(&self) -> Vec<(u32, Account<E>)>;
    fn block_number(&self) -> u32;
    fn root_hash(&self) -> E::Fr;
}

#[derive(Clone)]
pub struct Tx<E: JubjubEngine> {
    pub from:               E::Fr,
    pub to:                 E::Fr,
    pub amount:             E::Fr,
    pub fee:                E::Fr,
    pub nonce:              E::Fr,
    pub good_until_block:   E::Fr,
    pub signature:          TransactionSignature<E>,
}

impl <E: JubjubEngine> Tx<E> {
    pub fn public_data_into_bits(
        &self
    ) -> Vec<bool> {
        // fields are
        // - from
        // - to
        // - amount
        // - fee
        let mut from: Vec<bool> = BitIterator::new(self.from.into_repr()).collect();
        from.reverse();
        from.truncate(*plasma_constants::BALANCE_TREE_DEPTH);
        let mut to: Vec<bool> = BitIterator::new(self.to.into_repr()).collect();
        to.reverse();
        to.truncate(*plasma_constants::BALANCE_TREE_DEPTH);
        let mut amount: Vec<bool> = BitIterator::new(self.amount.into_repr()).collect();
        amount.reverse();
        amount.truncate(*plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH + *plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH);
        let mut fee: Vec<bool> = BitIterator::new(self.fee.into_repr()).collect();
        fee.reverse();
        fee.truncate(*plasma_constants::FEE_EXPONENT_BIT_WIDTH + *plasma_constants::FEE_MANTISSA_BIT_WIDTH);
        
        let mut packed: Vec<bool> = vec![];
        packed.extend(from.into_iter());
        packed.extend(to.into_iter());
        packed.extend(amount.into_iter());
        packed.extend(fee.into_iter());

        packed
    }

    pub fn data_for_signature_into_bits(
        &self
    ) -> Vec<bool> {
        // fields are
        // - from
        // - to
        // - amount
        // - fee
        // - nonce
        // - good_until_block
        let mut nonce: Vec<bool> = BitIterator::new(self.nonce.into_repr()).collect();
        nonce.reverse();
        nonce.truncate(*plasma_constants::NONCE_BIT_WIDTH);
        let mut good_until_block: Vec<bool> = BitIterator::new(self.good_until_block.into_repr()).collect();
        good_until_block.reverse();
        good_until_block.truncate(*plasma_constants::BLOCK_NUMBER_BIT_WIDTH);
        let mut packed: Vec<bool> = vec![];
        
        packed.extend(self.public_data_into_bits().into_iter());
        packed.extend(nonce.into_iter());
        packed.extend(good_until_block.into_iter());

        packed
    }
}

pub struct Block<E: JubjubEngine> {
    pub block_number:   u32,
    pub transactions:   Vec<Tx<E>>,
    pub new_root_hash:  E::Fr,
}
