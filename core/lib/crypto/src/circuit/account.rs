use lazy_static::lazy_static;

use crate::params;

use crate::franklin_crypto::bellman::pairing::ff::{Field, PrimeField};
use crate::franklin_crypto::bellman::pairing::Engine;

use crate::franklin_crypto::bellman::pairing::bn256::{Bn256, Fr};
use crate::franklin_crypto::rescue::RescueEngine;
use crate::merkle_tree::hasher::Hasher;
use crate::merkle_tree::{RescueHasher, SparseMerkleTree};
use crate::primitives::{GetBits, GetBitsFixed};

/// Account tree used in the `zksync_circuit`.
pub type CircuitAccountTree = SparseMerkleTree<CircuitAccount<Bn256>, Fr, RescueHasher<Bn256>>;
/// Balance tree for accounts used in the `zksync_circuit`.
pub type CircuitBalanceTree = SparseMerkleTree<Balance<Bn256>, Fr, RescueHasher<Bn256>>;

pub fn empty_account_as_field_elements<E: Engine>() -> Vec<E::Fr> {
    let acc = CircuitAccount::<Bn256>::default();
    let bits = acc.get_bits_le();

    use crate::franklin_crypto::circuit::multipack;

    multipack::compute_multipacking::<E>(&bits)
}

/// Representation of the zkSync account used in the `zksync_circuit`.
#[derive(Clone)]
pub struct CircuitAccount<E: RescueEngine> {
    pub subtree: SparseMerkleTree<Balance<E>, E::Fr, RescueHasher<E>>,
    pub nonce: E::Fr,
    pub pub_key_hash: E::Fr,
    pub address: E::Fr,
}

impl<E: RescueEngine> GetBits for CircuitAccount<E> {
    fn get_bits_le(&self) -> Vec<bool> {
        debug_assert_eq!(
            params::FR_BIT_WIDTH,
            E::Fr::NUM_BITS as usize,
            "FR bit width is not equal to field bit width"
        );
        let mut leaf_content = Vec::new();

        leaf_content.extend(self.nonce.get_bits_le_fixed(params::NONCE_BIT_WIDTH)); //32
        leaf_content.extend(
            self.pub_key_hash
                .get_bits_le_fixed(params::NEW_PUBKEY_HASH_WIDTH), //160
        );
        leaf_content.extend(
            self.address.get_bits_le_fixed(params::ADDRESS_WIDTH), //160
        );

        // calculate hash of the subroot using algebraic hash
        let state_root = self.get_state_root();

        let mut state_tree_hash_bits = state_root.get_bits_le_fixed(params::FR_BIT_WIDTH);
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

impl<E: RescueEngine> CircuitAccount<E> {
    fn get_state_root(&self) -> E::Fr {
        let balance_root = self.subtree.root_hash();

        let state_root_padding = E::Fr::zero();

        self.subtree
            .hasher
            .hash_elements(vec![balance_root, state_root_padding])
    }
}

impl std::default::Default for CircuitAccount<Bn256> {
    //default should be changed: since subtree_root_hash is not zero for all zero balances and subaccounts
    fn default() -> Self {
        Self {
            nonce: Fr::zero(),
            pub_key_hash: Fr::zero(),
            address: Fr::zero(),
            subtree: BALANCE_TREE.clone(),
        }
    }
}

/// Representation of one token balance used in `zksync_circuit`.
#[derive(Clone, Debug)]
pub struct Balance<E: Engine> {
    pub value: E::Fr,
}

impl<E: Engine> GetBits for Balance<E> {
    fn get_bits_le(&self) -> Vec<bool> {
        let mut leaf_content = Vec::new();
        leaf_content.extend(self.value.get_bits_le_fixed(params::BALANCE_BIT_WIDTH));
        assert!(
            params::BALANCE_BIT_WIDTH < E::Fr::CAPACITY as usize,
            "due to algebraic nature of the hash we should not overflow the capacity"
        );

        leaf_content
    }
}

impl<E: Engine> std::default::Default for Balance<E> {
    //default should be changed: since subtree_root_hash is not zero for all zero balances and subaccounts
    fn default() -> Self {
        Self {
            value: E::Fr::zero(),
        }
    }
}

lazy_static! {
    static ref BALANCE_TREE: CircuitBalanceTree =
        SparseMerkleTree::new(params::balance_tree_depth());
}
