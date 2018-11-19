#![feature(test)]

extern crate rand;
extern crate test;
extern crate pairing;
extern crate sapling_crypto;

use rand::{Rand, thread_rng};
use pairing::bls12_381::Bls12;
use sapling_crypto::jubjub::JubjubBls12;
use sapling_crypto::pedersen_hash::{pedersen_hash, Personalization};

#[bench]
fn bench_pedersen_hash(b: &mut test::Bencher) {
    let params = JubjubBls12::new();
    let rng = &mut thread_rng();
    let bits = (0..510).map(|_| bool::rand(rng)).collect::<Vec<_>>();
    let personalization = Personalization::MerkleTree(31);

    b.iter(|| {
        pedersen_hash::<Bls12, _>(personalization, bits.clone(), &params)
    });
}
