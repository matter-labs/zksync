use sapling_crypto::alt_babyjubjub::{JubjubEngine};
use ff::{Field, PrimeField};
use crate::models::params;
use crate::primitives::{GetBits, GetBitsFixed};

#[derive(Debug, Clone, Queryable, Serialize, Deserialize)]
pub struct Account<E: JubjubEngine> {
    pub balance:    E::Fr,
    pub nonce:      E::Fr,
    pub pub_x:      E::Fr,
    pub pub_y:      E::Fr,
}

impl<E: JubjubEngine> std::default::Default for Account<E> {
    fn default() -> Self{
        Self{
            balance:    E::Fr::zero(),
            nonce:      E::Fr::zero(),
            pub_x:      E::Fr::zero(),
            pub_y:      E::Fr::zero(),
        }
    }
}

impl<E: JubjubEngine> GetBits for Account<E> {
    fn get_bits_le(&self) -> Vec<bool> {
        let mut leaf_content = Vec::new();
        leaf_content.extend(self.balance.get_bits_le_fixed(params::BALANCE_BIT_WIDTH));
        leaf_content.extend(self.nonce.get_bits_le_fixed(params::NONCE_BIT_WIDTH));
        leaf_content.extend(self.pub_y.get_bits_le_fixed(params::FR_BIT_WIDTH - 1));
        leaf_content.extend(self.pub_x.get_bits_le_fixed(1));

        leaf_content
    }
}

// TODO: this is ugly; the correct way is to introduce Serialize/Deserialize interface into JubjubEngine::Fr
// this requires deduplication of JubjubEngines
impl std::convert::From<crate::models::Account> for Account<pairing::bn256::Bn256> {

    fn from(a: crate::models::Account) -> Self {
        use pairing::bn256::Fr;
        use crate::primitives::{unpack_edwards_point};

        let public_key = unpack_edwards_point::<pairing::bn256::Bn256>(a.public_key, &params::JUBJUB_PARAMS).unwrap();
        let (x, y) = public_key.into_xy();

        Self{
            balance:    Fr::from_str(&a.balance.to_string()).unwrap(),
            nonce:      Fr::from_str(&a.nonce.to_string()).unwrap(),
            pub_x:      x,
            pub_y:      y,
        }
    }

}