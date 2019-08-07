use ff::Field;
use franklin_crypto::alt_babyjubjub::JubjubEngine;
use franklin_crypto::jubjub::{edwards, Unknown};

// use crate::models::params;

#[derive(Clone, Serialize, Deserialize)]
pub struct TransactionSignature<E: JubjubEngine> {
    #[serde(bound = "")]
    pub r: edwards::Point<E, Unknown>,
    pub s: E::Fr,
}

impl<E: JubjubEngine> TransactionSignature<E> {
    pub fn empty() -> Self {
        let empty_point: edwards::Point<E, Unknown> = edwards::Point::zero();

        Self {
            r: empty_point,
            s: E::Fr::zero(),
        }
    }
}
