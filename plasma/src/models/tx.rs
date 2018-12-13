use sapling_crypto::alt_babyjubjub::{JubjubEngine};
use ff::{Field, PrimeField, BitIterator};
use sapling_crypto::eddsa::{PrivateKey, PublicKey};
use sapling_crypto::jubjub::{FixedGenerators, Unknown, edwards};
use crate::models::params;
use super::super::circuit::utils::{le_bit_vector_into_field_element};
use sapling_crypto::circuit::float_point::{convert_to_float}; // TODO: move to primitives

#[derive(Clone)]
pub struct TransactionSignature<E: JubjubEngine> {
    pub r: edwards::Point<E, Unknown>,
    pub s: E::Fr,
}
/// Packed transaction data
#[derive(Clone)]
pub struct Tx<E: JubjubEngine> {
    pub from:               E::Fr,
    pub to:                 E::Fr,
    pub amount:             E::Fr, // packed, TODO: document it here
    pub fee:                E::Fr, // packed
    pub nonce:              E::Fr,
    pub good_until_block:   E::Fr,
    pub signature:          TransactionSignature<E>,
}

/// Unpacked transaction data
#[derive(Debug, Clone)]
pub struct TxUnpacked{
    pub from:               u32,
    pub to:                 u32,
    pub amount:             u128,
    pub fee:                u128,
    pub nonce:              u32,
    pub good_until_block:   u32,

    pub sig_r:              String, // r.x
    pub sig_s:              String,
}

pub struct Block<E: JubjubEngine> {
    pub block_number:   u32,
    pub transactions:   Vec<Tx<E>>,
    pub new_root_hash:  E::Fr,
}


impl<E: JubjubEngine> TransactionSignature<E> {
    pub fn empty() -> Self {
        let empty_point: edwards::Point<E, Unknown> = edwards::Point::zero();
        Self{
            r: empty_point,
            s: E::Fr::zero()
        }
    }
}

impl<E: JubjubEngine> Tx<E> {

    // TODO: introduce errors if necessary
    pub fn try_from(transaction: &TxUnpacked) -> Result<Self, ()> {

        let encoded_amount_bits = convert_to_float(
            transaction.amount,
            params::AMOUNT_EXPONENT_BIT_WIDTH, 
            params::AMOUNT_MANTISSA_BIT_WIDTH, 
            10
        ).map_err(|_| ())?;
        let encoded_amount: E::Fr = le_bit_vector_into_field_element(&encoded_amount_bits);

        // TODO: encode fee
        let encoded_fee = E::Fr::zero();

        let tx = Self {
            // TODO: these conversions are ugly and inefficient, replace with idiomatic std::convert::From trait
            from:               E::Fr::from_str(&transaction.from.to_string()).unwrap(),
            to:                 E::Fr::from_str(&transaction.to.to_string()).unwrap(),
            amount:             encoded_amount,
            fee:                encoded_fee,
            nonce:              E::Fr::from_str(&transaction.good_until_block.to_string()).unwrap(),
            good_until_block:   E::Fr::from_str(&transaction.good_until_block.to_string()).unwrap(),

            // TODO: decode signature
            signature:          TransactionSignature::empty(),
        };

        Ok(tx)
    }

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
        from.truncate(params::BALANCE_TREE_DEPTH);
        let mut to: Vec<bool> = BitIterator::new(self.to.into_repr()).collect();
        to.reverse();
        to.truncate(params::BALANCE_TREE_DEPTH);
        let mut amount: Vec<bool> = BitIterator::new(self.amount.into_repr()).collect();
        amount.reverse();
        amount.truncate(params::AMOUNT_EXPONENT_BIT_WIDTH + params::AMOUNT_MANTISSA_BIT_WIDTH);
        let mut fee: Vec<bool> = BitIterator::new(self.fee.into_repr()).collect();
        fee.reverse();
        fee.truncate(params::FEE_EXPONENT_BIT_WIDTH + params::FEE_MANTISSA_BIT_WIDTH);
        
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
        nonce.truncate(params::NONCE_BIT_WIDTH);
        let mut good_until_block: Vec<bool> = BitIterator::new(self.good_until_block.into_repr()).collect();
        good_until_block.reverse();
        good_until_block.truncate(params::BLOCK_NUMBER_BIT_WIDTH);
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

        let converted_signature = TransactionSignature {
            r: signature.r,
            s: sigs_converted
        };

        self.signature = converted_signature;

    }
}