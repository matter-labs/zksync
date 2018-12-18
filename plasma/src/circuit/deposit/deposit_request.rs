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

use crate::models::params as plasma_constants;
use crate::circuit::utils::{le_bit_vector_into_field_element};

// This is deposit request

#[derive(Clone)]
pub struct DepositRequest<E: JubjubEngine> {
    pub into: Option<E::Fr>,
    pub amount: Option<E::Fr>,
    pub public_key: Option<edwards::Point<E, Unknown>>
}

impl<E: JubjubEngine> DepositRequest<E> {
    pub fn verify_public_key(
        &self,
        params: &E::Params,
    ) -> bool {
        {
            if self.public_key.is_none() {
                return false;
            }
        }
        let pk = self.public_key.clone().unwrap();
        let order_check = pk.mul(E::Fs::char(), params);
        order_check.eq(&Point::zero())
    }

    // this function returns public data in Ethereum compatible format
    pub fn public_data_into_bits(
        &self
    ) -> Vec<bool> {
        // fields are
        // - into
        // - amount
        // - compressed public key
        let mut into: Vec<bool> = BitIterator::new(self.into.clone().unwrap().into_repr()).collect();
        into.reverse();
        into.truncate(plasma_constants::BALANCE_TREE_DEPTH);
        // reverse again to have BE as in Ethereum native types
        into.reverse();

        let mut amount: Vec<bool> = BitIterator::new(self.amount.clone().unwrap().into_repr()).collect();
        amount.reverse();
        amount.truncate(plasma_constants::BALANCE_BIT_WIDTH);
        // reverse again to have BE as in Ethereum native types
        amount.reverse();

        // pack public key to reduce the amount of data
        let (y, sign_bit) = self.public_key.clone().unwrap().compress_into_y();
        let mut y_bits: Vec<bool> = BitIterator::new(y.into_repr()).collect();
        y_bits.reverse();
        y_bits.truncate(E::Fr::NUM_BITS as usize);
        y_bits.resize(plasma_constants::FR_BIT_WIDTH - 1, false);
        // push sign bit
        y_bits.push(sign_bit);
        // reverse again to have BE as in Ethereum native types
        y_bits.reverse();

        let mut packed: Vec<bool> = vec![];
        packed.extend(into.into_iter());
        packed.extend(amount.into_iter());
        packed.extend(y_bits.into_iter());

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