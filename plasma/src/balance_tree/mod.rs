// Plasma account (Merkle tree leaf)

use std::fmt::{self, Debug};
use ff::{Field, PrimeField, PrimeFieldRepr};
use rand::{Rand, thread_rng};
use pairing::bn256::{Bn256, Fr};
use sapling_crypto::alt_babyjubjub::{JubjubEngine, AltJubjubBn256, edwards::Point, PrimeOrder};

use super::primitives::{GetBits, GetBitsFixed};
use super::sparse_merkle_tree;
use super::sparse_merkle_tree::parallel_smt;
use super::sparse_merkle_tree::pedersen_hasher::PedersenHasher;

use super::circuit::plasma_constants;

#[derive(Debug, Clone)]
pub struct Leaf<E: JubjubEngine> {
    pub balance:    E::Fr,
    pub nonce:      E::Fr,
    pub pub_x:      E::Fr,
    pub pub_y:      E::Fr,
}

impl<E: JubjubEngine> GetBits for Leaf<E> {
    fn get_bits_le(&self) -> Vec<bool> {
        let mut leaf_content = Vec::new();
        leaf_content.extend(self.balance.get_bits_le_fixed(*plasma_constants::BALANCE_BIT_WIDTH));
        leaf_content.extend(self.nonce.get_bits_le_fixed(*plasma_constants::NONCE_BIT_WIDTH));
        leaf_content.extend(self.pub_x.get_bits_le_fixed(*plasma_constants::FR_BIT_WIDTH));
        leaf_content.extend(self.pub_y.get_bits_le_fixed(*plasma_constants::FR_BIT_WIDTH));
        leaf_content
    }
}

impl<E: JubjubEngine> Default for Leaf<E> {
    fn default() -> Self{
        Self {
            balance:    E::Fr::zero(),
            nonce:      E::Fr::zero(),
            pub_x:      E::Fr::zero(),
            pub_y:      E::Fr::zero(),
        }
    }
}

// code below is for testing

pub type BabyLeaf = Leaf<Bn256>;
pub type BabyBalanceTree = sparse_merkle_tree::SparseMerkleTree<BabyLeaf, Fr, PedersenHasher<Bn256>>;

impl BabyBalanceTree {
    pub fn verify_proof(&self, index: u32, item: BabyLeaf, proof: Vec<(Fr, bool)>) -> bool {
        use sparse_merkle_tree::hasher::Hasher;
        
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
            // print!("This level hash is {}\n", hash);
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
        let mut tree = BabyBalanceTree::new(3);
        let leaf = BabyLeaf {
            balance:    Fr::zero(),
            nonce:      Fr::one(),
            pub_x:      Fr::one(),
            pub_y:      Fr::one(),
        };
        tree.insert(3, leaf);
        let root = tree.root_hash();
        let path = tree.merkle_path(0);
    }


}
