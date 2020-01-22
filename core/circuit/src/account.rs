// External deps
use bellman::{ConstraintSystem, SynthesisError};
use franklin_crypto::circuit::Assignment;
use franklin_crypto::jubjub::JubjubEngine;
// Workspace deps
use crate::element::CircuitElement;

#[derive(Clone, Debug)]
pub struct AccountWitness<E: JubjubEngine> {
    pub nonce: Option<E::Fr>,
    pub pub_key_hash: Option<E::Fr>,
}

pub struct AccountContent<E: JubjubEngine> {
    pub nonce: CircuitElement<E>,
    pub pub_key_hash: CircuitElement<E>,
    pub address: CircuitElement<E>,
}

impl<E: JubjubEngine> AccountContent<E> {
    pub fn from_witness<CS: ConstraintSystem<E>>(
        mut cs: CS,
        witness: &AccountWitness<E>,
    ) -> Result<Self, SynthesisError> {
        let nonce = CircuitElement::from_fe_strict(
            cs.namespace(|| "nonce"),
            || Ok(witness.nonce.grab()?),
            models::params::NONCE_BIT_WIDTH,
        )?;

        let pub_key_hash = CircuitElement::from_fe_strict(
            cs.namespace(|| "pub_key_hash"),
            || witness.pub_key_hash.grab(),
            models::params::NEW_PUBKEY_HASH_WIDTH,
        )?;

        Ok(Self {
            nonce,
            pub_key_hash,
            address: unimplemented!("pay to eth circuit"),
        })
    }
}
