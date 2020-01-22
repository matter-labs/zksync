// External deps
use bellman::{ConstraintSystem, SynthesisError};
use franklin_crypto::circuit::Assignment;
use franklin_crypto::jubjub::JubjubEngine;
// Workspace deps
use crate::element::CircuitElement;
use models::circuit::account::CircuitAccount;

#[derive(Clone, Debug)]
pub struct AccountWitness<E: JubjubEngine> {
    pub nonce: Option<E::Fr>,
    pub pub_key_hash: Option<E::Fr>,
    pub address: Option<E::Fr>,
}

impl<E: JubjubEngine> AccountWitness<E> {
    pub fn from_circuit_account(circuit_account: &CircuitAccount<E>) -> Self {
        Self {
            nonce: Some(circuit_account.nonce.clone()),
            pub_key_hash: Some(circuit_account.pub_key_hash.clone()),
            address: Some(circuit_account.address.clone()),
        }
    }
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

        let address = CircuitElement::from_fe_strict(
            cs.namespace(|| "address"),
            || witness.address.grab(),
            models::params::ETHEREUM_KEY_BIT_WIDTH,
        )?;

        Ok(Self {
            nonce,
            pub_key_hash,
            address,
        })
    }
}
