// Plasma update circuit implementation

use ff::{Field, PrimeField};
use rand::{Rand, thread_rng};
use pairing::{Engine};
use bellman::{Circuit, ConstraintSystem, SynthesisError};

use sapling_crypto::jubjub::JubjubEngine;
use sapling_crypto::circuit::ecc::EdwardsPoint;
use sapling_crypto::alt_babyjubjub::AltJubjubBn256;
use sapling_crypto::pedersen_hash::{pedersen_hash, Personalization};

use super::plasma_state::TxPubInput;

//#[derive(Clone)]
//struct Leaf<E: JubjubEngine, TreeHeight: usize> {
//
//    // state: 4 field elements
//    balance:    E::Fs,
//    nonce:      E::Fs,
//    pubkey:     EdwardsPoint<E>,
//
//    // Merkle auth path
//    merkle_path: [E::Fs; TreeHeight],
//}
//
//struct TxWitness<E: JubjubEngine> {
//    pub_input:              TxPubInput,
//    from_leaf:              Leaf<E>,
//    to_leaf:                Leaf<E>,
//    sig:                    EdwardsPoint<E>,
//    merkle_root_updated:    E::Fs,
//}
//
////use sapling_crypto::circuit::sha256::{sha256};
////use sapling_crypto::circuit::num::{AllocatedNum};
////use sapling_crypto::circuit::multipack::{pack_into_inputs};
////use num_bigint::BigUint;
//
//struct PlasmaUpdateCircuit<E: Engine> {
//    final_hash: Option<E::Fr>,
//}
//
////    let params = JubJubEngine::new();
////    let rng = &mut thread_rng();
////    let bits = (0..510).map(|_| bool::rand(rng)).collect::<Vec<_>>();
////    let personalization = Personalization::MerkleTree(31);
////    pedersen_hash::<Bn256, _>(personalization, bits.clone(), &params);
//
//impl<E: Engine> Circuit<E> for PlasmaUpdateCircuit<E> {
//    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError> {
//
//        Ok(())
//    }
//}

pub fn test_circuit() {
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