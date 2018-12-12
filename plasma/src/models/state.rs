use std::collections::{hash_map, HashMap};
use sapling_crypto::alt_babyjubjub::{JubjubEngine};
use ff::{PrimeField, PrimeFieldRepr, BitIterator};
use sapling_crypto::eddsa::{PrivateKey, PublicKey};
use sapling_crypto::jubjub::{FixedGenerators, Unknown, edwards, JubjubParams};
use super::super::circuit::transfer::transaction::{TransactionSignature};
use super::super::circuit::utils::{le_bit_vector_into_field_element};

use super::account::Account;

pub trait State<E: JubjubEngine> {  
    fn get_accounts(&self) -> Vec<(u32, Account<E>)>;
    fn block_number(&self) -> u32;
    fn root_hash(&self) -> E::Fr;
}
