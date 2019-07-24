use franklin_crypto::jubjub::JubjubEngine;
use franklinmodels::params as franklin_constants;

use crate::element::{CircuitElement, CircuitPubkey};
use bellman::{ConstraintSystem, SynthesisError};
use franklin_crypto::circuit::Assignment;
#[derive(Clone, Debug)]
pub struct AccountWitness<E: JubjubEngine> {
    pub nonce: Option<E::Fr>,
    // x coordinate is supplied and parity is constrained
    pub pub_x: Option<E::Fr>,
    pub pub_y: Option<E::Fr>,
}

pub struct AccountContent<E: JubjubEngine> {
    pub nonce: CircuitElement<E>,
    pub pub_key: CircuitPubkey<E>,
}
impl<E: JubjubEngine> AccountContent<E> {
    pub fn from_witness<CS: ConstraintSystem<E>>(
        mut cs: CS,
        witness: &AccountWitness<E>,
        params: &E::Params,
    ) -> Result<Self, SynthesisError> {
        let nonce = CircuitElement::from_fe_strict(
            cs.namespace(|| "nonce"),
            || Ok(witness.nonce.grab()?),
            franklin_constants::NONCE_BIT_WIDTH,
        )?;
        let pub_key = CircuitPubkey::from_xy_fe(
            cs.namespace(|| "pub_key"),
            || Ok(witness.pub_x.grab()?),
            || Ok(witness.pub_y.grab()?),
            &params,
        )?;
        Ok(Self {
            nonce: nonce,
            pub_key: pub_key,
        })
    }
}
