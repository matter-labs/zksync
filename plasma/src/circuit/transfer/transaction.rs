use ff::{
    PrimeField,
    Field,
    BitIterator,
    PrimeFieldRepr
};

use bellman::{
    SynthesisError,
    ConstraintSystem,
    Circuit
};

use sapling_crypto::jubjub::{
    JubjubEngine,
    FixedGenerators,
    Unknown,
    edwards,
    JubjubParams
};

use super::Assignment;
use super::boolean;
use super::ecc;
use super::pedersen_hash;
use super::sha256;
use super::num;
use super::multipack;
use super::num::{AllocatedNum, Num};
use super::float_point::{parse_with_exponent_le, convert_to_float};
use super::baby_eddsa::EddsaSignature;

use sapling_crypto::eddsa::{
    Signature,
    PrivateKey,
    PublicKey
};

<<<<<<< HEAD
use crate::models::{params, circuit::sig::TransactionSignature};
use crate::circuit::utils::le_bit_vector_into_field_element;
=======
use super::super::plasma_constants;
use crate::circuit::utils::{le_bit_vector_into_field_element, allocate_audit_path, append_packed_public_key};
>>>>>>> more_ff

// This is transaction data

#[derive(Clone)]
pub struct Transaction<E: JubjubEngine> {
    pub from: Option<E::Fr>,
    pub to: Option<E::Fr>,
    pub amount: Option<E::Fr>,
    pub fee: Option<E::Fr>,
    pub nonce: Option<E::Fr>,
    pub good_until_block: Option<E::Fr>,
    pub signature: Option<TransactionSignature<E>>
}

pub struct TransactionContent<E: JubjubEngine> {
    pub amount_bits: Vec<boolean::Boolean>,
    pub fee_bits: Vec<boolean::Boolean>,
    pub good_until_block:AllocatedNum<E>
}


impl <E: JubjubEngine> Transaction<E> {
    // this function returns public transaction data in Ethereum compatible format
    pub fn public_data_into_bits(
        &self
    ) -> Vec<bool> {
        // fields are
        // - from
        // - to
        // - amount
        // - fee
        let mut from: Vec<bool> = BitIterator::new(self.from.clone().unwrap().into_repr()).collect();
        from.reverse();
<<<<<<< HEAD
        from.truncate(params::BALANCE_TREE_DEPTH);
        let mut to: Vec<bool> = BitIterator::new(self.to.clone().unwrap().into_repr()).collect();
        to.reverse();
        to.truncate(params::BALANCE_TREE_DEPTH);
=======
        from.truncate(*plasma_constants::BALANCE_TREE_DEPTH);
        // reverse again cause from and to are the only two fields that are kept BE
        from.reverse();
        let mut to: Vec<bool> = BitIterator::new(self.to.clone().unwrap().into_repr()).collect();
        to.reverse();
        to.truncate(*plasma_constants::BALANCE_TREE_DEPTH);
        // reverse again cause from and to are the only two fields that are kept BE
        to.reverse();
>>>>>>> more_ff
        let mut amount: Vec<bool> = BitIterator::new(self.amount.clone().unwrap().into_repr()).collect();
        amount.reverse();
        amount.truncate(params::AMOUNT_EXPONENT_BIT_WIDTH + params::AMOUNT_MANTISSA_BIT_WIDTH);
        let mut fee: Vec<bool> = BitIterator::new(self.fee.clone().unwrap().into_repr()).collect();
        fee.reverse();
        fee.truncate(params::FEE_EXPONENT_BIT_WIDTH + params::FEE_MANTISSA_BIT_WIDTH);
        
        let mut packed: Vec<bool> = vec![];
        packed.extend(from.into_iter());
        packed.extend(to.into_iter());
        packed.extend(amount.into_iter());
        packed.extend(fee.into_iter());

        packed
    }

    // this function returns data to make a transaction signature
    // in a format that is later used in zkSNARK
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

        // in data for signature and for latter use in SNARKs everything is LE!

        // LE from
        let mut from: Vec<bool> = BitIterator::new(self.from.clone().unwrap().into_repr()).collect();
        from.reverse();
        from.truncate(*plasma_constants::BALANCE_TREE_DEPTH);
        // LE to
        let mut to: Vec<bool> = BitIterator::new(self.to.clone().unwrap().into_repr()).collect();
        to.reverse();
        to.truncate(*plasma_constants::BALANCE_TREE_DEPTH);
        // amount is encoded as float
        let mut amount: Vec<bool> = BitIterator::new(self.amount.clone().unwrap().into_repr()).collect();
        amount.reverse();
        amount.truncate(*plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH + *plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH);
        // same for fee
        let mut fee: Vec<bool> = BitIterator::new(self.fee.clone().unwrap().into_repr()).collect();
        fee.reverse();
        fee.truncate(*plasma_constants::FEE_EXPONENT_BIT_WIDTH + *plasma_constants::FEE_MANTISSA_BIT_WIDTH); 
        // nonce is LE encoded
        let mut nonce: Vec<bool> = BitIterator::new(self.nonce.clone().unwrap().into_repr()).collect();
        nonce.reverse();
<<<<<<< HEAD
        nonce.truncate(params::NONCE_BIT_WIDTH);
        let mut good_until_block: Vec<bool> = BitIterator::new(self.good_until_block.clone().unwrap().into_repr()).collect();
        good_until_block.reverse();
        good_until_block.truncate(params::BLOCK_NUMBER_BIT_WIDTH);
=======
        nonce.truncate(*plasma_constants::NONCE_BIT_WIDTH);
        // LE good until block #
        let mut good_until_block: Vec<bool> = BitIterator::new(self.good_until_block.clone().unwrap().into_repr()).collect();
        good_until_block.reverse();
        good_until_block.truncate(*plasma_constants::BLOCK_NUMBER_BIT_WIDTH);

>>>>>>> more_ff
        let mut packed: Vec<bool> = vec![];

        packed.extend(from.into_iter());
        packed.extend(to.into_iter());
        packed.extend(amount.into_iter());
        packed.extend(fee.into_iter());
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

        let max_message_len = params::BALANCE_TREE_DEPTH 
                        + params::BALANCE_TREE_DEPTH 
                        + params::AMOUNT_EXPONENT_BIT_WIDTH 
                        + params::AMOUNT_MANTISSA_BIT_WIDTH
                        + params::FEE_EXPONENT_BIT_WIDTH
                        + params::FEE_MANTISSA_BIT_WIDTH
                        + params::NONCE_BIT_WIDTH
                        + params::BLOCK_NUMBER_BIT_WIDTH;
        
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

        // let mut sigs_bytes = [0u8; 32];
        // signature.s.into_repr().write_le(& mut sigs_bytes[..]).expect("get LE bytes of signature S");
        // let mut sigs_repr = E::Fr::zero().into_repr();
        // sigs_repr.read_le(&sigs_bytes[..]).expect("interpret S as field element representation");
        // let sigs_converted = E::Fr::from_repr(sigs_repr).unwrap();

        let converted_signature = TransactionSignature {
            r: signature.r,
            s: sigs_converted
        };

        self.signature = Some(converted_signature);

    }
}