use crate::franklin_crypto::bellman::groth16::{
    create_random_proof, prepare_verifying_key, verify_proof, Parameters, Proof,
};
use crate::franklin_crypto::bellman::{Circuit, SynthesisError};
use crate::node::{Engine, Fr};
use crate::primitives::{serialize_g1_for_ethereum, serialize_g2_for_ethereum};
use crate::EncodedProof;
use crypto_exports::rand::thread_rng;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

#[derive(Debug)]
pub struct FullBabyProof {
    pub proof: Proof<Engine>,
    pub public_input: Fr,
}

/// Prepare proof for Ethereum
pub fn encode_proof(proof: &Proof<Engine>) -> EncodedProof {
    // proof
    // pub a: E::G1Affine,
    // pub b: E::G2Affine,
    // pub c: E::G1Affine

    let (a_x, a_y) = serialize_g1_for_ethereum(proof.a);

    let ((b_x_0, b_x_1), (b_y_0, b_y_1)) = serialize_g2_for_ethereum(proof.b);

    let (c_x, c_y) = serialize_g1_for_ethereum(proof.c);

    [a_x, a_y, b_x_0, b_x_1, b_y_0, b_y_1, c_x, c_y]
}

pub fn verify_full_baby_proof(
    proof: &FullBabyProof,
    circuit_params: &Parameters<Engine>,
) -> Result<bool, SynthesisError> {
    let pvk = prepare_verifying_key(&circuit_params.vk);
    verify_proof(&pvk, &proof.proof, &[proof.public_input])
}

pub fn create_random_full_baby_proof<C: Circuit<Engine>>(
    circuit_instance: C,
    public_input: Fr,
    circuit_params: &Parameters<Engine>,
) -> Result<FullBabyProof, SynthesisError> {
    let proof = create_random_proof(circuit_instance, circuit_params, &mut thread_rng())?;
    Ok(FullBabyProof {
        proof,
        public_input,
    })
}

pub fn read_circuit_proving_parameters<P: AsRef<Path>>(
    file_name: P,
) -> std::io::Result<Parameters<Engine>> {
    let f_r = File::open(file_name)?;
    let mut r = BufReader::new(f_r);
    Parameters::<Engine>::read(&mut r, true)
}
