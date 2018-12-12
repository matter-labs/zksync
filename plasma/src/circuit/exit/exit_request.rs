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
    JubjubParams,
    edwards::Point,
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

use sapling_crypto::alt_babyjubjub::*;

use crate::models::params;
use crate::circuit::utils::{le_bit_vector_into_field_element};

// This is an exit request

#[derive(Clone)]
pub struct ExitRequest<E: JubjubEngine> {
    pub from: Option<E::Fr>,
    // keep an amount in request for ease of public data serialization
    pub amount: Option<E::Fr>,
}

impl<E: JubjubEngine> ExitRequest<E> {
    pub fn public_data_into_bits(
        &self
    ) -> Vec<bool> {
        // fields are
        // - from
        // - amount
        // - compressed public key
        let mut from: Vec<bool> = BitIterator::new(self.from.clone().unwrap().into_repr()).collect();
        from.reverse();
        from.truncate(params::BALANCE_TREE_DEPTH);
        // reverse again to have BE as in Ethereum native types
        from.reverse();

        let mut amount: Vec<bool> = BitIterator::new(self.amount.clone().unwrap().into_repr()).collect();
        amount.reverse();
        amount.truncate(params::BALANCE_BIT_WIDTH);
        // reverse again to have BE as in Ethereum native types
        amount.reverse();

        let mut packed: Vec<bool> = vec![];
        packed.extend(from.into_iter());
        packed.extend(amount.into_iter());

        packed
    }

    pub fn data_as_bytes(
        & self
    ) -> Vec<u8> {
        let raw_data: Vec<bool> = self.public_data_into_bits();

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
}