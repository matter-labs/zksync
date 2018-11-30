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

#[bench]
fn bench_balance_tree_update_once(b: &mut Bencher) {
    bench_balance_tree_update(b, 1);
}

#[bench]
fn bench_balance_tree_update_100(b: &mut Bencher) {
    bench_balance_tree_update(b, 100);
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

    b.iter(|| {
        let insert_into = u32::rand(rng) % capacity;
        for i in 0..leafs.len() {
            tree.insert(u32::rand(rng) % capacity, leafs[i].clone())
        }
        tree.root_hash()
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

