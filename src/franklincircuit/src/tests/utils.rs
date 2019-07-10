
use franklin_crypto::eddsa::{PrivateKey, PublicKey};
use franklinmodels::params as franklin_constants;

use crate::account::*;
use crate::circuit::FranklinCircuit;
use crate::operation::*;
use crate::utils::*;
use bellman::Circuit;
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use ff::Field;
use ff::{BitIterator, PrimeField, PrimeFieldRepr};
use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use franklin_crypto::circuit::float_point::convert_to_float;
use franklin_crypto::circuit::test::*;
use franklin_crypto::jubjub::FixedGenerators;
use franklinmodels::circuit::account::{Balance, CircuitAccount,CircuitAccountTree, CircuitBalanceTree};
use merkle_tree::hasher::Hasher;
use merkle_tree::PedersenHasher;
use pairing::bn256::*;
use rand::{Rng, SeedableRng, XorShiftRng};



pub struct DepositWitness{

}
pub fn deposit(leaf: CircuitBalanceTree, state_tree: CircuitAccountTree) -> DepositWitness {
    unimplemented!()
}
