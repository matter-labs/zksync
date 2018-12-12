use std::collections::{hash_map, HashMap};
use sapling_crypto::alt_babyjubjub::{JubjubEngine};
use ff::{Field, PrimeField, PrimeFieldRepr, BitIterator};
use sapling_crypto::eddsa::{PrivateKey, PublicKey};
use sapling_crypto::jubjub::{FixedGenerators, Unknown, edwards, JubjubParams};
use crate::models::params;
use crate::circuit::transfer::transaction::{TransactionSignature};
use crate::circuit::utils::{le_bit_vector_into_field_element};

use crate::primitives::{GetBits, GetBitsFixed};

#[derive(Debug, Clone)]
pub struct Account<E: JubjubEngine> {
    pub balance:    E::Fr,
    pub nonce:      E::Fr,
    pub pub_x:      E::Fr,
    pub pub_y:      E::Fr,
}


//////////////////////////////////////////////////////


impl<E: JubjubEngine> GetBits for Account<E> {
    fn get_bits_le(&self) -> Vec<bool> {
        let mut leaf_content = Vec::new();
        leaf_content.extend(self.balance.get_bits_le_fixed(params::BALANCE_BIT_WIDTH));
        leaf_content.extend(self.nonce.get_bits_le_fixed(params::NONCE_BIT_WIDTH));
        leaf_content.extend(self.pub_x.get_bits_le_fixed(params::FR_BIT_WIDTH));
        leaf_content.extend(self.pub_y.get_bits_le_fixed(params::FR_BIT_WIDTH));
        leaf_content
    }
}

impl<E: JubjubEngine> Default for Account<E> {
    fn default() -> Self{
        Self {
            balance:    E::Fr::zero(),
            nonce:      E::Fr::zero(),
            pub_x:      E::Fr::zero(),
            pub_y:      E::Fr::zero(),
        }
    }
}
