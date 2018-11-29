extern crate rand;
extern crate pairing;
extern crate bellman;
extern crate plasma;
extern crate sapling_crypto;

use rand::thread_rng;
use bellman::groth16::{
    create_random_proof, generate_random_parameters, prepare_verifying_key, verify_proof,
};
use pairing::bn256::{Bn256, Fr};

use plasma::vk_contract_generator::generate_vk_contract;
use plasma::circuit::baby_plasma::Update;

use sapling_crypto::alt_babyjubjub::AltJubjubBn256;

// Create some parameters, create a proof, and verify the proof.
fn main() {

    let rng = &mut thread_rng();
    let params = {
        let params = &AltJubjubBn256::new();
        let c = Update::<Bn256> {
            params,
            number_of_transactions: 0,
            old_root: None,
            new_root: None,
            public_data_commitment: None,
            block_number: None,
            total_fee: None,
            transactions: vec![],
        };
        generate_random_parameters(c, rng).unwrap()
    };

    let pvk = prepare_verifying_key(&params.vk);
    println!("{}", generate_vk_contract(&params.vk));
}
