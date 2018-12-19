use sapling_crypto::alt_babyjubjub::{JubjubEngine};
use ff::{Field, PrimeField, PrimeFieldRepr, BitIterator};
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
    pub fn try_from(
        sig: crate::models::TxSignature,
        params: &E::Params
    ) -> Result<Self, String> {
        // TxSignature has S and R in compressed form serialized as BE
        let x_sign = sig.r_compressed[0] & 0x80 > 0;
        let mut tmp = sig.r_compressed.clone();
        tmp[0] &= 0x7f; // strip the top bit

        // read from byte arrays
        let y_repr = E::Fr::zero().into_repr();
        y_repr.read_be(&tmp[..]).expect("read R_y as field element");

        let s_repr = E::Fr::zero().into_repr();
        s_repr.read_be(&sig.s[..]).expect("read S as field element");

        let y = E::Fr::from_repr(y_repr).expect("make y from representation");

        // here we convert it to field elements for all further uses
        let r = edwards::Point::get_for_y(y, x_sign, params);
        if r.is_none() {
            return Err("Invalid R point".to_string());
        }

        let s = E::Fr::from_repr(s_repr).expect("make s from representation");

        Ok(Self{
            r: r.unwrap(),
            s: s,
        })
    }
}