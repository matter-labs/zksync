#![feature(test)]

extern crate ff;
extern crate rand;
extern crate test;
extern crate plasma;
extern crate pairing;

use ff::{Field};
use rand::{Rand, thread_rng};
use test::Bencher;

use pairing::bn256::{Fr};
use plasma::balance_tree::*;

#[bench]
fn bench_balance_tree_update(b: &mut Bencher) {

    const N_INSERTS: usize = 100;

    let rng = &mut thread_rng();
    let mut tree = BabyBalanceTree::new(24);
    let leaf = BabyLeaf {
        balance:    Fr::zero(),
        nonce:      Fr::one(),
        pub_x:      Fr::one(),
        pub_y:      Fr::one(),
    };

    let capacity = tree.capacity();
    let mut leafs = Vec::with_capacity(N_INSERTS);
    leafs.extend((0..N_INSERTS).map(|_| BabyLeaf {
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