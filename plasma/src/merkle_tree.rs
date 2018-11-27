// Merkle tree on Pedersen hashes

use ff::{Field, PrimeField};
use rand::{Rand, thread_rng};
use pairing::{Engine};

use sapling_crypto::jubjub::JubjubEngine;

pub trait Hashable<E: JubjubEngine> {
    fn hash(&self) -> E::Fs;
}

#[derive(Debug, Clone)]
pub struct MerkleTree<E: JubjubEngine> {

    depth: usize,
    items: Vec<E::Fs>,

    // hashes
    //merkle_root: E::Fs,
}

impl<E: JubjubEngine> MerkleTree<E> {

    fn new(depth: usize) -> Self {

        assert!(depth > 0 && depth < 32);

        let items = vec![E::Fs::zero(); Self::capacity(depth)];

        //let merkle_root = E::Fs::zero();

        Self{depth, items}//.update_hashes()
    }

    fn capacity(tree_height: usize) -> usize {
        2 << tree_height
    }

//    fn update_hashes(&mut self) -> &Self {
//        self
//    }
}
