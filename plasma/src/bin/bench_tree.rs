#![feature(test)]

extern crate ff;
extern crate rand;
extern crate test;
extern crate plasma;
extern crate pairing;
extern crate time;

use ff::{Field, PrimeField, BitIterator};
use rand::{Rand, thread_rng, XorShiftRng, SeedableRng};
use test::Bencher;

use pairing::bn256::{Fr};
use plasma::balance_tree::*;
use plasma::primitives::*;
use plasma::sparse_merkle_tree::batched_smt;
use plasma::sparse_merkle_tree::pedersen_hasher::BabyPedersenHasher;


fn main() {

    let n_inserts = 10000;
    let rounds: usize = 1;
    let height: usize = 24;

    let mut rng = &mut XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
    let mut leafs = Vec::with_capacity(n_inserts * rounds);
    let rand_leaf = |_| BabyLeaf {
        balance:    Fr::rand(rng),
        nonce:      Fr::rand(rng),
        pub_x:      Fr::rand(rng),
        pub_y:      Fr::rand(rng),
    };
    leafs.extend((0..(n_inserts * rounds)).map(rand_leaf));

    let mut rng = &mut XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
    let mut pos = Vec::with_capacity(n_inserts * rounds);
    pos.extend((0..(n_inserts * rounds)).map(|_| usize::rand(rng)));

    println!("running {} rounds of {} inserts each...", rounds, n_inserts);

    let mut tree = BabyBalanceTree::new(height as u32);
    let mut v1 = Vec::new();
    let mut dummy = 0;
    let capacity = tree.capacity() as usize;

    println!("start");
    let start = time::now();
    for j in 0..rounds {
        for i in 0..n_inserts {
            let insert_into = pos[i] % capacity;
            let value = leafs[n_inserts*j + i].clone();
            tree.insert(insert_into as u32, value);
        }
        v1.push(tree.root_hash());
    }
    println!("default done in {}", (time::now() - start));

    type BTree = batched_smt::SparseMerkleTree<BabyLeaf, Fr, BabyPedersenHasher>;

    let mut tree = BTree::new(height);
    let mut v2 = Vec::new();
    let mut dummy = 0;
    let capacity = tree.capacity();

    BTree::reset_stats();
    println!("start");
    let start = time::now();
    for j in 0..rounds {
        for i in 0..n_inserts {
            let insert_into = pos[i] % capacity;
            let value = leafs[n_inserts*j + i].clone();
            tree.insert(insert_into, value);
        }
        v2.push(tree.root_hash());
    }
    println!("batch done in {}", (time::now() - start));
    BTree::print_stats();

    assert_eq!(v1, v2);
    println!("test ok");
}
