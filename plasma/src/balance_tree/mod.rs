// Plasma account (Merkle tree leaf)

use std::fmt::{self, Debug};
use ff::{Field, PrimeField, PrimeFieldRepr};
use rand::{Rand, thread_rng};
use pairing::bn256::{Bn256, Fr};
use sapling_crypto::babyjubjub::{JubjubEngine, JubjubBn256, edwards::Point, PrimeOrder};

use super::smt::SparseMerkleTree;
use super::smt::hasher::IntoBits;
use super::smt::pedersen_hasher::BabyPedersenHasher;

use sapling_crypto::circuit::matter::plasma_constants;

#[derive(Debug, Clone)]
pub struct Leaf<E: JubjubEngine> {
    balance:    E::Fr,
    nonce:      E::Fr,
    pub_x:      E::Fr,
    pub_y:      E::Fr,
}

fn get_bits<E: JubjubEngine>(value: E::Fr, n: usize) -> Vec<bool> {
    let mut acc = Vec::with_capacity(n);
    let mut t = value.into_repr().clone();
    for i in 0..n {
        acc.push(t.is_odd());
        t.shr(1);
    }
    acc
}

impl<E: JubjubEngine> IntoBits for Leaf<E> {
    fn into_bits(&self) -> Vec<bool> {
        let mut leaf_content = Vec::new();
        leaf_content.extend(get_bits::<E>(self.balance, *plasma_constants::BALANCE_BIT_WIDTH));
        leaf_content.extend(get_bits::<E>(self.nonce, *plasma_constants::NONCE_BIT_WIDTH));
        leaf_content.extend(get_bits::<E>(self.pub_x, *plasma_constants::FR_BIT_WIDTH));
        leaf_content.extend(get_bits::<E>(self.pub_y, *plasma_constants::FR_BIT_WIDTH));
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
    println!("root: {:?}", root);
}
