use sapling_crypto::jubjub::JubjubEngine;

use super::boolean;
use super::num::AllocatedNum;
use super::Assignment;
use bellman::{ConstraintSystem, SynthesisError};

use crate::circuit::utils::append_packed_public_key;
use crate::models::params as plasma_constants;

#[derive(Clone)]
pub struct LeafWitness<E: JubjubEngine> {
    pub balance: Option<E::Fr>,
    pub nonce: Option<E::Fr>,
    // x coordinate is supplied and parity is constrained
    pub pub_x: Option<E::Fr>,
    pub pub_y: Option<E::Fr>,
}

pub struct LeafContent<E: JubjubEngine> {
    pub leaf_bits: Vec<boolean::Boolean>,
    pub value: AllocatedNum<E>,
    pub value_bits: Vec<boolean::Boolean>,
    pub nonce: AllocatedNum<E>,
    pub nonce_bits: Vec<boolean::Boolean>,
    pub pub_x: AllocatedNum<E>,
    pub pub_y: AllocatedNum<E>,
    pub pub_x_bit: Vec<boolean::Boolean>,
    pub pub_y_bits: Vec<boolean::Boolean>,
}

pub fn make_leaf_content<E, CS>(
    mut cs: CS,
    witness: LeafWitness<E>,
) -> Result<LeafContent<E>, SynthesisError>
where
    E: JubjubEngine,
    CS: ConstraintSystem<E>,
{
    let mut leaf_bits = vec![];

    let value = AllocatedNum::alloc(cs.namespace(|| "allocate leaf value witness"), || {
        Ok(*witness.balance.get()?)
    })?;

    let mut value_bits = value.into_bits_le(cs.namespace(|| "value bits"))?;

    value_bits.truncate(plasma_constants::BALANCE_BIT_WIDTH);
    leaf_bits.extend(value_bits.clone());

    let nonce = AllocatedNum::alloc(cs.namespace(|| "allocate leaf nonce witness"), || {
        Ok(*witness.nonce.get()?)
    })?;

    let mut nonce_bits = nonce.into_bits_le(cs.namespace(|| "nonce bits"))?;

    nonce_bits.truncate(plasma_constants::NONCE_BIT_WIDTH);
    leaf_bits.extend(nonce_bits.clone());

    // we allocate (witness) public X and Y to use them also later for signature check

    let pub_x = AllocatedNum::alloc(cs.namespace(|| "allocate public key x witness"), || {
        Ok(*witness.pub_x.get()?)
    })?;

    let pub_y = AllocatedNum::alloc(cs.namespace(|| "allcoate public key y witness"), || {
        Ok(*witness.pub_y.get()?)
    })?;

    let mut pub_x_bit = pub_x.into_bits_le(cs.namespace(|| "pub_x bits"))?;
    // leave only the parity bit
    pub_x_bit.truncate(1);

    let mut pub_y_bits = pub_y.into_bits_le(cs.namespace(|| "pub_y bits"))?;
    pub_y_bits.resize(
        plasma_constants::FR_BIT_WIDTH - 1,
        boolean::Boolean::Constant(false),
    );

    append_packed_public_key(&mut leaf_bits, pub_x_bit.clone(), pub_y_bits.clone());

    // leaf_bits.extend(pub_y_bits);
    // leaf_bits.extend(pub_x_bit);

    assert_eq!(
        leaf_bits.len(),
        plasma_constants::BALANCE_BIT_WIDTH
            + plasma_constants::NONCE_BIT_WIDTH
            + plasma_constants::FR_BIT_WIDTH
    );

    Ok(LeafContent {
        leaf_bits: leaf_bits,
        value: value,
        value_bits: value_bits,
        nonce: nonce,
        nonce_bits: nonce_bits,
        pub_x: pub_x,
        pub_y: pub_y,
        pub_x_bit: pub_x_bit,
        pub_y_bits: pub_y_bits,
    })
}
