// Plasma account (Merkle tree leaf)

use std::fmt::{self, Debug};
use ff::{Field, PrimeField, PrimeFieldRepr};
use rand::{Rand, thread_rng};
use pairing::bn256::{Bn256, Fr};
use sapling_crypto::alt_babyjubjub::{JubjubEngine, AltJubjubBn256, edwards::Point, PrimeOrder};

use super::primitives::{GetBits, GetBitsFixed};
use crate::sparse_merkle_tree;
use crate::sparse_merkle_tree::pedersen_hasher::PedersenHasher;

const HASH_LENGTH: usize = 256;

#[derive(Debug, Clone)]
pub struct Leaf<E: JubjubEngine>{
    pub hash:    Vec<bool>,
    pub phantom: std::marker::PhantomData<E>,
}

impl<E: JubjubEngine> GetBits for Leaf<E> {
    fn get_bits_le(&self) -> Vec<bool> {
        self.hash.clone()
    }
}

impl<E: JubjubEngine> Default for Leaf<E> {
    fn default() -> Self{
        let mut v = Vec::with_capacity(HASH_LENGTH);
        v.resize(HASH_LENGTH, false);
        Self {
            hash: v,
            phantom: std::marker::PhantomData
        }
    }
}

// code below is for testing

pub type BabyTransactionLeaf = Leaf<Bn256>;
pub type BabyTransactionTree = sparse_merkle_tree::SparseMerkleTree<BabyTransactionLeaf, Fr, PedersenHasher<Bn256>>;

impl BabyTransactionTree {
    pub fn verify_proof(&self, index: u32, item: BabyTransactionLeaf, proof: Vec<(Fr, bool)>) -> bool {
        use crate::sparse_merkle_tree::hasher::Hasher;
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
    use rand::{Rand, thread_rng};

    #[test]
    fn test_balance_tree() {
        let mut tree = BabyTransactionTree::new(3);
        let leaf = BabyTransactionLeaf::default();
        tree.insert(3, leaf);
        let root = tree.root_hash();
        let path = tree.merkle_path(0);
    }


}
