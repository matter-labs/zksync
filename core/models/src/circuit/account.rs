use crate::params;

use franklin_crypto::bellman::pairing::ff::{Field, PrimeField, PrimeFieldRepr};
use franklin_crypto::alt_babyjubjub::JubjubEngine;

use crate::merkle_tree::{PedersenHasher, SparseMerkleTree};
use crate::primitives::{GetBits, GetBitsFixed};
use franklin_crypto::bellman::pairing::bn256::{Bn256, Fr};
pub type CircuitAccountTree = SparseMerkleTree<CircuitAccount<Bn256>, Fr, PedersenHasher<Bn256>>;
pub type CircuitBalanceTree = SparseMerkleTree<Balance<Bn256>, Fr, PedersenHasher<Bn256>>;
pub struct CircuitAccount<E: JubjubEngine> {
    pub subtree: SparseMerkleTree<Balance<E>, E::Fr, PedersenHasher<E>>,
    pub nonce: E::Fr,
    pub pub_key_hash: E::Fr,
}

impl<E: JubjubEngine> GetBits for CircuitAccount<E> {
    fn get_bits_le(&self) -> Vec<bool> {
        let mut leaf_content = Vec::new();

        leaf_content.extend(self.nonce.get_bits_le_fixed(params::NONCE_BIT_WIDTH)); //32
        leaf_content.extend(
            self.pub_key_hash
                .get_bits_le_fixed(params::NEW_PUBKEY_HASH_WIDTH), //160
        );

        let mut root_hash_bits = self
            .subtree
            .root_hash()
            .get_bits_le_fixed(params::FR_BIT_WIDTH);
        root_hash_bits.resize(params::FR_BIT_WIDTH_PADDED, false); //256

        leaf_content.extend(root_hash_bits);

        leaf_content
    }
}

impl<E: JubjubEngine> CircuitAccount<E> {
    //we temporary pass it as repr. TODO: return Fr, when we could provide proper trait bound
    pub fn empty_balances_root_hash() -> Vec<u8> {
        let balances_smt = CircuitBalanceTree::new(params::BALANCE_TREE_DEPTH as u32);
        let mut tmp = [0u8; 32];
        balances_smt
            .root_hash()
            .into_repr()
            .write_be(&mut tmp[..])
            .unwrap();
        tmp.to_vec()
    }
}

impl std::default::Default for CircuitAccount<Bn256> {
    //default should be changed: since subtree_root_hash is not zero for all zero balances and subaccounts
    fn default() -> Self {
        Self {
            nonce: Fr::zero(),
            pub_key_hash: Fr::zero(),
            subtree: SparseMerkleTree::new(params::BALANCE_TREE_DEPTH as u32),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Balance<E: JubjubEngine> {
    pub value: E::Fr,
}

impl<E: JubjubEngine> GetBits for Balance<E> {
    fn get_bits_le(&self) -> Vec<bool> {
        let mut leaf_content = Vec::new();
        leaf_content.extend(self.value.get_bits_le_fixed(params::BALANCE_BIT_WIDTH));

        leaf_content
    }
}

impl<E: JubjubEngine> std::default::Default for Balance<E> {
    //default should be changed: since subtree_root_hash is not zero for all zero balances and subaccounts
    fn default() -> Self {
        Self {
            value: E::Fr::zero(),
        }
    }
}
