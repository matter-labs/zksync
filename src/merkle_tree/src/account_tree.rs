// Plasma account (Merkle tree leaf)
// TODO: - Not used in project

use ff::Field;
use pairing::bn256::{Bn256, Fr};
use sapling_crypto::alt_babyjubjub::JubjubEngine;

use crate::merkle_tree::PedersenHasher;
use crate::merkle_tree::SparseMerkleTree;
use crate::models::params as plasma_constants;
use crate::primitives::{GetBits, GetBitsFixed};

#[derive(Debug, Clone)]
pub struct Leaf<E: JubjubEngine> {
    pub balance: E::Fr,
    pub nonce: E::Fr,
    pub pub_x: E::Fr,
    pub pub_y: E::Fr,
}

impl<E: JubjubEngine> GetBits for Leaf<E> {
    fn get_bits_le(&self) -> Vec<bool> {
        let mut leaf_content = Vec::new();
        leaf_content.extend(
            self.balance
                .get_bits_le_fixed(plasma_constants::BALANCE_BIT_WIDTH),
        );
        leaf_content.extend(
            self.nonce
                .get_bits_le_fixed(plasma_constants::NONCE_BIT_WIDTH),
        );
        leaf_content.extend(
            self.pub_y
                .get_bits_le_fixed(plasma_constants::FR_BIT_WIDTH - 1),
        );
        leaf_content.extend(self.pub_x.get_bits_le_fixed(1));

        leaf_content
    }
}

impl<E: JubjubEngine> Default for Leaf<E> {
    fn default() -> Self {
        Self {
            balance: E::Fr::zero(),
            nonce: E::Fr::zero(),
            pub_x: E::Fr::zero(),
            pub_y: E::Fr::zero(),
        }
    }
}

// code below is for testing

pub type LeafAccount = Leaf<Bn256>;
pub type LeafAccountTree = SparseMerkleTree<LeafAccount, Fr, PedersenHasher<Bn256>>;

impl LeafAccountTree {
    pub fn verify_proof(&self, index: u32, item: LeafAccount, proof: Vec<(Fr, bool)>) -> bool {
        use crate::merkle_tree::hasher::Hasher;

        assert!(index < self.capacity());
        let item_bits = item.get_bits_le();
        let mut hash = self.hasher.hash_bits(item_bits);
        let mut proof_index: u32 = 0;

        for (i, e) in proof.clone().into_iter().enumerate() {
            if e.1 {
                // current is right
                proof_index |= 1 << i;
                hash = self.hasher.compress(&e.0, &hash, i);
            } else {
                // current is left
                hash = self.hasher.compress(&hash, &e.0, i);
            }
        }

        if proof_index != index {
            return false;
        }

        hash == self.root_hash()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_balance_tree() {
        let mut tree = LeafAccountTree::new(3);
        let leaf = LeafAccount {
            balance:    Fr::zero(),
            nonce:      Fr::one(),
            pub_x:      Fr::one(),
            pub_y:      Fr::one(),
        };
        tree.insert(3, leaf);
        let _root = tree.root_hash();
        let _path = tree.merkle_path(0);
    }

}
