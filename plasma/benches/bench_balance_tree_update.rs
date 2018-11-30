#![feature(test)]

extern crate ff;
extern crate rand;
extern crate test;
extern crate plasma;
extern crate pairing;

use ff::{Field, PrimeField, BitIterator};
use rand::{Rand, thread_rng};
use test::Bencher;

use pairing::bn256::{Fr};
use plasma::balance_tree::*;
use plasma::primitives::*;
use plasma::sparse_merkle_tree::batching;
use plasma::sparse_merkle_tree::pedersen_hasher::BabyPedersenHasher;


#[bench]
fn bench_balance_tree_update_once(b: &mut Bencher) {
    bench_balance_tree_update(b, 1);
}

#[bench]
fn bench_tree_update_100_1(b: &mut Bencher) {
    bench_balance_tree_update(b, 10);
}

#[bench]
fn bench_tree_update_100_batched(b: &mut Bencher) {
    bench_batched_smt(b, 10);
}

fn bench_balance_tree_update(b: &mut Bencher, n_inserts: usize) {
    let rng = &mut thread_rng();
    let mut tree = BabyBalanceTree::new(24);
    let capacity = tree.capacity();
    let mut leafs = Vec::with_capacity(n_inserts);
    leafs.extend((0..n_inserts).map(|_| BabyLeaf {
        balance:    Fr::rand(rng),
        nonce:      Fr::rand(rng),
        pub_x:      Fr::rand(rng),
        pub_y:      Fr::rand(rng),
    }));

    let insert_into = u32::rand(rng) % capacity;
    for i in 0..leafs.len() {
        tree.insert(insert_into, leafs[i].clone())
    }

    b.iter(|| {
        tree.root_hash()
    });
}

fn bench_batched_smt(b: &mut Bencher, n_inserts: usize) {

    type BabyBalanceTree = batching::SparseMerkleTree<BabyLeaf, Fr, BabyPedersenHasher>;

    let rng = &mut thread_rng();
    let mut tree = BabyBalanceTree::new(24);
    let capacity = tree.capacity();
    let mut leafs = Vec::with_capacity(n_inserts);
    leafs.extend((0..n_inserts).map(|_| BabyLeaf {
        balance:    Fr::rand(rng),
        nonce:      Fr::rand(rng),
        pub_x:      Fr::rand(rng),
        pub_y:      Fr::rand(rng),
    }));

    b.iter(|| {
        for i in 0..leafs.len() {
            let insert_into = usize::rand(rng) % capacity;
            tree.insert(insert_into, leafs[i].clone());
        }
        tree.root_hash();
    });
}

//#[bench]
//fn bench_bit_iter(b: &mut Bencher) {
//    let rng = &mut thread_rng();
//    let fr = Fr::rand(rng);
//
//    b.iter(|| {
//        //let mut input: Vec<bool> = Vec::with_capacity(2 * Fr::NUM_BITS as usize);
//        //input.extend(BitIterator::new(fr.into_repr()));
//        //input
//        BitIterator::new(fr.into_repr()).last()
//    });
//}
//
//#[bench]
//fn bench_bit_iter_concat(b: &mut Bencher) {
//    let rng = &mut thread_rng();
//    let fr = Fr::rand(rng);
//
//    b.iter(|| {
//        let mut input: Vec<bool> = Vec::with_capacity(2 * Fr::NUM_BITS as usize);
//        input.extend(fr.get_bits_le_fixed(Fr::NUM_BITS as usize));
//        input.extend(fr.get_bits_le_fixed(Fr::NUM_BITS as usize));
//        input
//    });
//}

