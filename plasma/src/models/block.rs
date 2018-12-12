use std::collections::{hash_map, HashMap};
use sapling_crypto::alt_babyjubjub::{JubjubEngine};
use ff::{PrimeField, PrimeFieldRepr, BitIterator};
use sapling_crypto::eddsa::{PrivateKey, PublicKey};
use sapling_crypto::jubjub::{FixedGenerators, Unknown, edwards, JubjubParams};
use super::super::circuit::transfer::transaction::{TransactionSignature};
use super::super::circuit::utils::{le_bit_vector_into_field_element};

use super::tx::Tx;

pub struct Block<E: JubjubEngine> {
    pub block_number:   u32,
    pub transactions:   Vec<Tx<E>>,
    pub new_root_hash:  E::Fr,
}
