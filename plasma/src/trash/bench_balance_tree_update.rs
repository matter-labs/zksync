#![feature(test)]

extern crate ff;
extern crate rand;
extern crate test;
extern crate plasma;
extern crate pairing;

use rand::{Rand, thread_rng};
use test::Bencher;

use pairing::bn256::{Fr};
use plasma::balance_tree::*;
use plasma::sparse_merkle_tree::parallel_smt;
use plasma::sparse_merkle_tree::pedersen_hasher::BabyPedersenHasher;


#[bench]
fn bench_tree_update_once(b: &mut Bencher) {
    bench_balance_tree_update(b, 1);
}

#[bench]
fn bench_tree_update_100(b: &mut Bencher) {
    bench_balance_tree_update(b, 100);
}

#[bench]
fn bench_tree_update_100_batched_once(b: &mut Bencher) {
    bench_batched_smt(b, 1);
}

#[bench]
fn bench_tree_update_100_batched(b: &mut Bencher) {
    bench_batched_smt(b, 100);
}

fn bench_balance_tree_update(b: &mut Bencher, n_inserts: usize) {
    let rng = &mut thread_rng();
    let mut tree = AccountTree::new(24);
    let capacity = tree.capacity();
    let mut leafs = Vec::with_capacity(n_inserts);
    leafs.extend((0..n_inserts).map(|_| Account {
        balance:    Fr::rand(rng),
        nonce:      Fr::rand(rng),
        pub_x:      Fr::rand(rng),
        pub_y:      Fr::rand(rng),
    }));

    b.iter(|| {
        for i in 0..leafs.len() {
            let insert_into = u32::rand(rng) % capacity;
            tree.insert(insert_into, leafs[i].clone())
        }
        tree.root_hash()
    });
}

fn bench_batched_smt(b: &mut Bencher, n_inserts: usize) {

    type AccountTree = parallel_smt::SparseMerkleTree<Account, Fr, BabyPedersenHasher>;

    let rng = &mut thread_rng();
    let mut tree = AccountTree::new(24);
    let capacity = tree.capacity();
    let mut leafs = Vec::with_capacity(n_inserts);
    leafs.extend((0..n_inserts).map(|_| Account {
        balance:    Fr::rand(rng),
        nonce:      Fr::rand(rng),
        pub_x:      Fr::rand(rng),
        pub_y:      Fr::rand(rng),
    }));

    b.iter(|| {
        for i in 0..n_inserts {
            let insert_into = usize::rand(rng) % capacity;
            tree.insert(insert_into, leafs[i].clone());
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

