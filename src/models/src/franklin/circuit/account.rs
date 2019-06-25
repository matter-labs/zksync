use crate::franklin::params;
use crate::primitives::{GetBits, GetBitsFixed};
use ff::{Field, PrimeField};
use franklin_crypto::alt_babyjubjub::JubjubEngine;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitAccount<E: JubjubEngine> {
    pub subtree_root_hash: E::Fr,
    pub nonce: E::Fr,
    pub pub_x: E::Fr,
    pub pub_y: E::Fr,
}

impl<E: JubjubEngine> std::default::Default for CircuitAccount<E> {
    fn default() -> Self {
        Self {
            subtree_root_hash: E::Fr::zero(),
            nonce: E::Fr::zero(),
            pub_x: E::Fr::zero(),
            pub_y: E::Fr::zero(),
        }
    }
}

impl<E: JubjubEngine> GetBits for CircuitAccount<E> {
    fn get_bits_le(&self) -> Vec<bool> {
        let mut leaf_content = Vec::new();
        //TODO: verify_order
        leaf_content.extend(self.subtree_root_hash.get_bits_le_fixed(*params::FR_BIT_WIDTH));
        leaf_content.extend(self.nonce.get_bits_le_fixed(*params::NONCE_BIT_WIDTH));
        leaf_content.extend(self.pub_y.get_bits_le_fixed(params::FR_BIT_WIDTH - 1));
        leaf_content.extend(self.pub_x.get_bits_le_fixed(1));

        leaf_content
    }
}

