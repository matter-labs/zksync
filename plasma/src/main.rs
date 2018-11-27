#![allow(unused_imports)]
#![allow(unused_variables)]
extern crate bellman;
extern crate pairing;
extern crate rand;
extern crate hex;
extern crate ff;
extern crate sapling_crypto;

mod plasma_state;
mod merkle_tree;
mod update_circuit;

use merkle_tree::MerkleTree;

fn main() {
    //let plasma_params = plasma_state::PlasmaBN256{};
    update_circuit::test_circuit();
}
