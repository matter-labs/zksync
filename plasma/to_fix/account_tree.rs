// Plasma account (Merkle tree leaf)

use std::fmt::{self, Debug};
use ff::{Field, PrimeField, PrimeFieldRepr};
use rand::{Rand, thread_rng};
use pairing::bn256::{Bn256, Fr};
use sapling_crypto::alt_babyjubjub::{JubjubEngine, AltJubjubBn256, edwards::Point, PrimeOrder};

use crate::primitives::{GetBits, GetBitsFixed};
use crate::sparse_merkle_tree;
use crate::sparse_merkle_tree::parallel_smt;
use crate::sparse_merkle_tree::pedersen_hasher::PedersenHasher;
use crate::models::params;

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
<<<<<<< HEAD:plasma/to_fix/account_tree.rs
        leaf_content.extend(self.balance.get_bits_le_fixed(params::BALANCE_BIT_WIDTH));
        leaf_content.extend(self.nonce.get_bits_le_fixed(params::NONCE_BIT_WIDTH));
        leaf_content.extend(self.pub_x.get_bits_le_fixed(params::FR_BIT_WIDTH));
        leaf_content.extend(self.pub_y.get_bits_le_fixed(params::FR_BIT_WIDTH));
=======
        leaf_content.extend(self.balance.get_bits_le_fixed(*plasma_constants::BALANCE_BIT_WIDTH));
        leaf_content.extend(self.nonce.get_bits_le_fixed(*plasma_constants::NONCE_BIT_WIDTH));
        leaf_content.extend(self.pub_y.get_bits_le_fixed(*plasma_constants::FR_BIT_WIDTH - 1));
        leaf_content.extend(self.pub_x.get_bits_le_fixed(1));
        // for b in leaf_content.clone() {
        //     if b {
        //         print!("1");
        //     } else {
        //         print!("0");
        //     }
        // }
        // print!("\n");

>>>>>>> more_ff:plasma/src/balance_tree/mod.rs
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

pub type Account = Leaf<Bn256>;
pub type AccountTree = sparse_merkle_tree::SparseMerkleTree<Account, Fr, PedersenHasher<Bn256>>;

impl AccountTree {
    pub fn verify_proof(&self, index: u32, item: Account, proof: Vec<(Fr, bool)>) -> bool {
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
        let mut tree = AccountTree::new(3);
        let leaf = Account {
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
