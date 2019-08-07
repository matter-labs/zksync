use crate::plasma::params::{self, ETH_TOKEN_ID};
use crate::primitives::{GetBits, GetBitsFixed};
use ff::{Field, PrimeField};
use franklin_crypto::alt_babyjubjub::JubjubEngine;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitAccount<E: JubjubEngine> {
    pub balance: E::Fr,
    pub nonce: E::Fr,
    pub pub_x: E::Fr,
    pub pub_y: E::Fr,
}

impl<E: JubjubEngine> std::default::Default for CircuitAccount<E> {
    fn default() -> Self {
        Self {
            balance: E::Fr::zero(),
            nonce: E::Fr::zero(),
            pub_x: E::Fr::zero(),
            pub_y: E::Fr::zero(),
        }
    }
}

impl<E: JubjubEngine> GetBits for CircuitAccount<E> {
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
impl std::convert::From<crate::plasma::Account> for CircuitAccount<pairing::bn256::Bn256> {
    fn from(a: crate::plasma::Account) -> Self {
        use pairing::bn256::Fr;

        unimplemented!()
        //        Self {
        //            balance: Fr::from_str(&a.get_balance(ETH_TOKEN_ID).to_string()).unwrap(),
        //            nonce: Fr::from_str(&a.nonce.to_string()).unwrap(),
        //            pub_x: a.public_key_x,
        //            pub_y: a.public_key_y,
        //        }
    }
}
