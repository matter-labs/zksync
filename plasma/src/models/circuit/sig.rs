use sapling_crypto::alt_babyjubjub::{JubjubEngine};
use ff::{Field, PrimeField, PrimeFieldRepr, BitIterator};
use sapling_crypto::eddsa::{PrivateKey, PublicKey};
use sapling_crypto::jubjub::{FixedGenerators, Unknown, edwards};
use crate::circuit::utils::{le_bit_vector_into_field_element};
use sapling_crypto::circuit::float_point::{convert_to_float}; // TODO: move to primitives

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

        Self{
            r: empty_point,
            s: E::Fr::zero()
        }
    }
}

impl<E: JubjubEngine> TransactionSignature<E> {
    pub fn try_from(
        sig: crate::models::tx::TxSignature,
        params: &E::Params
    ) -> Result<Self, String> {
        let r = edwards::Point::from_xy(sig.r_x, sig.r_y, params).expect("make R point");
        let s = sig.s;
        
        Ok(Self{
            r: r,
            s: s,
        })
    }
}