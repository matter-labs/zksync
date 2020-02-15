use crate::params;

use crate::franklin_crypto::alt_babyjubjub::JubjubEngine;
use crate::franklin_crypto::bellman::pairing::ff::{Field, PrimeField, PrimeFieldRepr};

use crate::franklin_crypto::bellman::pairing::bn256::{Bn256, Fr};
use crate::merkle_tree::hasher::Hasher;
use crate::merkle_tree::{PedersenHasher, SparseMerkleTree};
use crate::primitives::{GetBits, GetBitsFixed};

pub type CircuitAccountTree = SparseMerkleTree<CircuitAccount<Bn256>, Fr, PedersenHasher<Bn256>>;
pub type CircuitBalanceTree = SparseMerkleTree<Balance<Bn256>, Fr, PedersenHasher<Bn256>>;
pub struct CircuitAccount<E: JubjubEngine> {
    pub subtree: SparseMerkleTree<Balance<E>, E::Fr, PedersenHasher<E>>,
    pub nonce: E::Fr,
    pub pub_key_hash: E::Fr,
    pub address: E::Fr,
}

impl<E: JubjubEngine> GetBits for CircuitAccount<E> {
    fn get_bits_le(&self) -> Vec<bool> {
        let mut leaf_content = Vec::new();

        leaf_content.extend(self.nonce.get_bits_le_fixed(params::NONCE_BIT_WIDTH)); //32
        leaf_content.extend(
            self.pub_key_hash
                .get_bits_le_fixed(params::NEW_PUBKEY_HASH_WIDTH), //160
        );
        leaf_content.extend(
            self.address.get_bits_le_fixed(params::ADDRESS_WIDTH), //160
        );

        let mut balance_root_bits = self
            .subtree
            .root_hash()
            .get_bits_le_fixed(params::FR_BIT_WIDTH);
        balance_root_bits.resize(params::FR_BIT_WIDTH_PADDED, false); //256

        // In future some other subtree can be added here instead of the empty hash.
        let state_root_bits = vec![false; params::FR_BIT_WIDTH_PADDED];

        let mut subtree_hash_input_bits = Vec::with_capacity(params::FR_BIT_WIDTH_PADDED * 2);
        subtree_hash_input_bits.extend(balance_root_bits.into_iter());
        subtree_hash_input_bits.extend(state_root_bits.into_iter());

        let mut state_tree_hash_bits = self
            .subtree
            .hasher
            .hash_bits(subtree_hash_input_bits.into_iter())
            .get_bits_le_fixed(params::FR_BIT_WIDTH);
        state_tree_hash_bits.resize(params::FR_BIT_WIDTH_PADDED, false);

        leaf_content.extend(state_tree_hash_bits.into_iter());
        assert_eq!(
            leaf_content.len(),
            params::LEAF_DATA_BIT_WIDTH,
            "Account bit width mismatch"
        );
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
            address: Fr::zero(),
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
