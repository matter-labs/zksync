use super::operation::TransactionSignature;
use crate::account::AccountContent;
use crate::account::AccountWitness;
use crate::allocated_structures::*;
use crate::element::{CircuitElement, CircuitPubkey};
use crate::operation::{Operation, SignatureData};
use crate::utils::{allocate_numbers_vec, allocate_sum, pack_bits_to_element};
use bellman::{Circuit, ConstraintSystem, SynthesisError};
use ff::{Field, PrimeField, PrimeFieldRepr};
use franklin_crypto::circuit::baby_eddsa::EddsaSignature;
use franklin_crypto::circuit::boolean::{AllocatedBit, Boolean};
use franklin_crypto::circuit::ecc;
use franklin_crypto::circuit::sha256;

use franklin_crypto::circuit::expression::Expression;
use franklin_crypto::circuit::num::AllocatedNum;
use franklin_crypto::circuit::pedersen_hash;
use franklin_crypto::circuit::polynomial_lookup::{do_the_lookup, generate_powers};
use franklin_crypto::circuit::Assignment;
use franklin_crypto::jubjub::{FixedGenerators, JubjubEngine, JubjubParams};
use models::circuit::account::CircuitAccount;
use models::params as franklin_constants;

pub struct AllocatedSignatureData<E: JubjubEngine> {
    pub eddsa: EddsaSignature<E>,
    pub is_verified: Boolean,
    pub sig_r_y_bits: Vec<Boolean>,
    pub sig_r_x_bit: Boolean,
    pub sig_s_bits: Vec<Boolean>,
}
