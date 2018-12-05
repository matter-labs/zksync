use std::collections::{hash_map, HashMap};

use sapling_crypto::alt_babyjubjub::{JubjubEngine};

use ff::{PrimeField, PrimeFieldRepr, BitIterator};

use super::super::circuit::plasma_constants;
use super::super::balance_tree;
use super::super::circuit::baby_plasma::TransactionSignature;
use sapling_crypto::eddsa::{PrivateKey, PublicKey};
use sapling_crypto::jubjub::{
    FixedGenerators,
    Unknown,
    edwards,
    JubjubParams
};

use super::super::circuit::utils::{le_bit_vector_into_field_element};

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

    pub fn data_as_bytes(
        & self
    ) -> Vec<u8> {
        let raw_data: Vec<bool> = self.data_for_signature_into_bits();

        let mut message_bytes: Vec<u8> = vec![];

        let byte_chunks = raw_data.chunks(8);
        for byte_chunk in byte_chunks
        {
            let mut byte = 0u8;
            for (i, bit) in byte_chunk.into_iter().enumerate()
            {
                if *bit {
                    byte |= 1 << i;
                }
            }
            message_bytes.push(byte);
        }

        message_bytes
    }

    pub fn sign<R>(
        & mut self,
        private_key: &PrivateKey<E>,
        p_g: FixedGenerators,
        params: &E::Params,
        rng: & mut R
    ) where R: rand::Rng {

        let message_bytes = self.data_as_bytes();

        let max_message_len = *plasma_constants::BALANCE_TREE_DEPTH 
                        + *plasma_constants::BALANCE_TREE_DEPTH 
                        + *plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH 
                        + *plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH
                        + *plasma_constants::FEE_EXPONENT_BIT_WIDTH
                        + *plasma_constants::FEE_MANTISSA_BIT_WIDTH
                        + *plasma_constants::NONCE_BIT_WIDTH
                        + *plasma_constants::BLOCK_NUMBER_BIT_WIDTH;
        
        let signature = private_key.sign_raw_message(
            &message_bytes, 
            rng, 
            p_g, 
            params,
            max_message_len / 8
        );

        let pk = PublicKey::from_private(&private_key, p_g, params);
        let is_valid_signature = pk.verify_for_raw_message(&message_bytes, 
                                        &signature.clone(), 
                                        p_g, 
                                        params, 
                                        max_message_len/8);
        if !is_valid_signature {
            return;
        }

        let mut sigs_le_bits: Vec<bool> = BitIterator::new(signature.s.into_repr()).collect();
        sigs_le_bits.reverse();

        let sigs_converted = le_bit_vector_into_field_element(&sigs_le_bits);

        let converted_signature = TransactionSignature {
            r: signature.r,
            s: sigs_converted
        };

        self.signature = converted_signature;

    }
}

pub struct Block<E: JubjubEngine> {
    pub block_number:   u32,
    pub transactions:   Vec<Tx<E>>,
    pub new_root_hash:  E::Fr,
}
