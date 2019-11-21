use franklin_crypto::jubjub::JubjubEngine;
use models::params as franklin_constants;

use crate::element::CircuitElement;
use bellman::{ConstraintSystem, SynthesisError};
use franklin_crypto::circuit::Assignment;
#[derive(Clone, Debug)]
pub struct AccountWitness<E: JubjubEngine> {
    pub nonce: Option<E::Fr>,
    pub pub_key_hash: Option<E::Fr>,
    pub address: Option<E::Fr>,
}

pub struct AccountContent<E: JubjubEngine> {
    pub nonce: CircuitElement<E>,
    pub pub_key_hash: CircuitElement<E>,
}
impl<E: JubjubEngine> AccountContent<E> {
    pub fn from_witness<CS: ConstraintSystem<E>>(
        mut cs: CS,
        witness: &AccountWitness<E>,
    ) -> Result<Self, SynthesisError> {
        let nonce = CircuitElement::from_fe_strict(
            cs.namespace(|| "nonce"),
            || Ok(witness.nonce.grab()?),
            franklin_constants::NONCE_BIT_WIDTH,
        )?;

        let pub_key_hash = CircuitElement::from_fe_strict(
            cs.namespace(|| "pub_key_hash"),
            || witness.pub_key_hash.grab(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        )?;

        Ok(Self {
            nonce,
            pub_key_hash,
        })
    }
}
