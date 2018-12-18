use sapling_crypto::alt_babyjubjub::{JubjubEngine};
use ff::{Field, PrimeField, BitIterator};
use sapling_crypto::eddsa::{PrivateKey, PublicKey};
use sapling_crypto::jubjub::{FixedGenerators, Unknown, edwards};
use crate::circuit::utils::{le_bit_vector_into_field_element};
use sapling_crypto::circuit::float_point::{convert_to_float}; // TODO: move to primitives

use crate::models::params;

#[derive(Clone, Serialize, Deserialize)]
pub struct TransactionSignature<E: JubjubEngine> {
    #[serde(bound = "")]
    pub r: edwards::Point<E, Unknown>,
    pub s: E::Fr,
}

impl<E: JubjubEngine> TransactionSignature<E> {
    pub fn empty() -> Self {
        let empty_point: edwards::Point<E, Unknown> = edwards::Point::zero();
        Self{
            r: empty_point,
            s: E::Fr::zero()
        }
    }
}

impl<E: JubjubEngine> TransactionSignature<E> {
    pub fn try_from(sig: crate::models::TxSignature) -> Result<Self, String> {
        unimplemented!()
        // Self{
        //     r:  edwards::Point{
        //             x: E::Fr::from_str(&sig.r_x.to_string()).unwrap(),
        //             y: E::Fr::from_str(&sig.r_y.to_string()).unwrap(),
        //             t: E::Fr::zero(),
        //             z: E::Fr::zero(),
        //             _marker: std::marker::PhantomData,
        //         },
        //     s:  E::Fr::from_str(&sig.s.to_string()).unwrap(),
        // }
    }
}