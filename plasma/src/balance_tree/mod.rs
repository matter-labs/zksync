// Plasma account (Merkle tree leaf)

use std::fmt::{self, Debug};
use ff::{Field, PrimeField, PrimeFieldRepr};
use rand::{Rand, thread_rng};
use pairing::bn256::{Bn256, Fr};
use sapling_crypto::babyjubjub::{JubjubEngine, JubjubBn256, edwards::Point, PrimeOrder};

use super::primitives::{get_bits_le, IntoBits};
use super::sparse_merkle_tree::SparseMerkleTree;
use super::sparse_merkle_tree::pedersen_hasher::BabyPedersenHasher;

use super::circuit::plasma_constants;

#[derive(Debug, Clone)]
pub struct Leaf<E: JubjubEngine> {
    pub balance:    E::Fr,
    pub nonce:      E::Fr,
    pub pub_x:      E::Fr,
    pub pub_y:      E::Fr,
}

impl<E: JubjubEngine> IntoBits for Leaf<E> {
    fn into_bits(&self) -> Vec<bool> {
        let mut leaf_content = Vec::new();
        leaf_content.extend(get_bits_le(self.balance, *plasma_constants::BALANCE_BIT_WIDTH));
        leaf_content.extend(get_bits_le(self.nonce, *plasma_constants::NONCE_BIT_WIDTH));
        leaf_content.extend(get_bits_le(self.pub_x, *plasma_constants::FR_BIT_WIDTH));
        leaf_content.extend(get_bits_le(self.pub_y, *plasma_constants::FR_BIT_WIDTH));
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

pub type BabyLeaf = Leaf<Bn256>;

pub type BabyBalanceTree = SparseMerkleTree<BabyLeaf, Fr, BabyPedersenHasher>;

#[test]
fn test_account_merkle_tree() {
    let mut tree = BabyBalanceTree::new(3);
    let leaf = BabyLeaf {
        balance:    Fr::zero(),
        nonce:      Fr::one(),
        pub_x:      Fr::one(),
        pub_y:      Fr::one(),
    };
    tree.insert(0, leaf);
    let root = tree.root_hash();
    //println!("root: {:?}", root);

    let path = tree.merkle_path(0);
    //println!("path: {:?}", path);
}
