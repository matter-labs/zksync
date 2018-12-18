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
            balance: E::Fr::zero(),
            nonce: E::Fr::zero(),
            pub_x: E::Fr::zero(),
            pub_y: E::Fr::zero(),
        }
    }
}

impl<E: JubjubEngine> GetBits for Account<E> {
    fn get_bits_le(&self) -> Vec<bool> {
        let mut leaf_content = Vec::new();
        leaf_content.extend(self.balance.get_bits_le_fixed(params::BALANCE_BIT_WIDTH));
        leaf_content.extend(self.nonce.get_bits_le_fixed(params::NONCE_BIT_WIDTH));
        leaf_content.extend(self.pub_x.get_bits_le_fixed(params::FR_BIT_WIDTH));
        leaf_content.extend(self.pub_y.get_bits_le_fixed(params::FR_BIT_WIDTH));
        leaf_content
    }
}

impl<E: JubjubEngine> std::convert::From<crate::models::Account> for Account<E> {

    fn from(a: crate::models::Account) -> Self {
        Self{
            balance:    E::Fr::from_str(&a.balance.to_string()).unwrap(),
            nonce:      E::Fr::from_str(&a.nonce.to_string()).unwrap(),
            pub_x:      E::Fr::from_str(&a.pub_x.into_repr().to_string()).unwrap(),
            pub_y:      E::Fr::from_str(&a.pub_y.into_repr().to_string()).unwrap(),
        }
    }

}