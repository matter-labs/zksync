// External deps
use zksync_crypto::franklin_crypto::{
    bellman::{ConstraintSystem, SynthesisError},
    circuit::Assignment,
};
// Workspace deps
use crate::element::CircuitElement;
use zksync_crypto::circuit::account::CircuitAccount;
use zksync_crypto::franklin_crypto::rescue::RescueEngine;

#[derive(Clone, Debug)]
pub struct AccountWitness<E: RescueEngine> {
    pub nonce: Option<E::Fr>,
    pub pub_key_hash: Option<E::Fr>,
    pub address: Option<E::Fr>,
}

impl<E: RescueEngine> AccountWitness<E> {
    pub fn from_circuit_account(circuit_account: &CircuitAccount<E>) -> Self {
        Self {
            nonce: Some(circuit_account.nonce),
            pub_key_hash: Some(circuit_account.pub_key_hash),
            address: Some(circuit_account.address),
        }
    }
}

pub struct AccountContent<E: RescueEngine> {
    pub nonce: CircuitElement<E>,
    pub pub_key_hash: CircuitElement<E>,
    pub address: CircuitElement<E>,
}

impl<E: RescueEngine> AccountContent<E> {
    pub fn from_witness<CS: ConstraintSystem<E>>(
        mut cs: CS,
        witness: &AccountWitness<E>,
    ) -> Result<Self, SynthesisError> {
        let nonce = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "nonce"),
            || Ok(witness.nonce.grab()?),
            zksync_crypto::params::NONCE_BIT_WIDTH,
        )?;

        let pub_key_hash = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "pub_key_hash"),
            || witness.pub_key_hash.grab(),
            zksync_crypto::params::NEW_PUBKEY_HASH_WIDTH,
        )?;

        let address = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "address"),
            || witness.address.grab(),
            zksync_crypto::params::ETH_ADDRESS_BIT_WIDTH,
        )?;

        Ok(Self {
            nonce,
            pub_key_hash,
            address,
        })
    }
}
