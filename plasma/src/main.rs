#![allow(unused_imports)]
#![allow(unused_variables)]
extern crate bellman;
extern crate pairing;
extern crate rand;
extern crate hex;
extern crate ff;
extern crate sapling_crypto;

fn main() {

//    use rand::thread_rng;
//    use pairing::bn256::{Bn256, Fr};
//    use bellman::groth16::{
//        create_random_proof, generate_random_parameters, prepare_verifying_key, verify_proof,
//    };
//
//    let rng = &mut thread_rng();
//
//    let params = {
//        let c = PlasmaUpdateCircuit::<Bn256> {
//            final_hash: None,
//        };
//        generate_random_parameters(c, rng).unwrap()
//    };
//
//    let pvk = prepare_verifying_key(&params.vk);
//
//    let c = PlasmaUpdateCircuit {
//        final_hash: Some(Fr::zero()),
//    };
//
//    let proof = create_random_proof(c, &params, rng).unwrap();
//
//    let inputs = &[
//    ];
//    let success = verify_proof(&pvk, &proof, inputs).unwrap();
//    assert!(success);

}
