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
use time::precise_time_ns;

use pairing::bn256::{Fr};
use plasma::balance_tree::*;
use plasma::primitives::*;
use plasma::sparse_merkle_tree::batched_smt;
use plasma::sparse_merkle_tree::pedersen_hasher::BabyPedersenHasher;


fn main() {

    let n_inserts = 1000;
    let rounds: usize = 10;
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

    let total = rounds * n_inserts;
    println!("running {} rounds of {} updates each (total {} updates):", rounds, n_inserts, total);

    let mut tree = BabyBalanceTree::new(height as u32);
    let mut v1 = Vec::new();
    let mut dummy = 0;
    let capacity = tree.capacity() as usize;

    println!("baseline implementation...");
    let start = precise_time_ns();
    for j in 0..rounds {
        for i in 0..n_inserts {
            let insert_into = pos[n_inserts*j + i] % capacity;
            let value = leafs[n_inserts*j + i].clone();
            tree.insert(insert_into as u32, value);
        }
        v1.push(tree.root_hash());
    }
    let duration = (precise_time_ns() - start) as f64 / 1_000_000_000.0;
    println!("done in {} s @ {} TPS", duration, total as f64 / duration);

    type BTree = batched_smt::SparseMerkleTree<BabyLeaf, Fr, BabyPedersenHasher>;

    let mut tree = BTree::new(height);
    let mut v2 = Vec::new();
    let mut dummy = 0;
    let capacity = tree.capacity();

    BTree::reset_stats();
    println!("parallel batched updates...");
    let start = precise_time_ns();
    for j in 0..rounds {
        for i in 0..n_inserts {
            let insert_into = pos[n_inserts*j + i] % capacity;
            let value = leafs[n_inserts*j + i].clone();
            tree.insert(insert_into, value);
        }
        v2.push(tree.root_hash());
    }
    let duration = (precise_time_ns() - start) as f64 / 1_000_000_000.0;
    println!("done in {} s @ {} TPS", duration, total as f64 / duration);
    BTree::print_stats();

    assert_eq!(v1, v2);
    println!("results match: all ok");
}
