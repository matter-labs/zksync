use crate::franklin_crypto::bellman::groth16::{
    create_random_proof, prepare_verifying_key, verify_proof, Parameters, Proof,
};
use crate::franklin_crypto::bellman::{Circuit, SynthesisError};
use crate::node::{Engine, Fr};
use crate::params::{account_tree_depth, BALANCE_TREE_DEPTH};
use crate::primitives::{serialize_g1_for_ethereum, serialize_g2_for_ethereum};
use crate::EncodedProof;
use crypto_exports::rand::thread_rng;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

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
const KEY_FILENAME: &str = "zksync_pk.key";
const EXIT_KEY_FILENAME: &str = "zksync_exit_pk.key";
const VERIFY_KEY_FILENAME: &str = "GetVk.sol";

pub fn get_keys_root_dir() -> PathBuf {
    let mut out_dir = PathBuf::new();
    out_dir.push(&std::env::var("ZKSYNC_HOME").unwrap_or_else(|_| "/".to_owned()));
    out_dir.push(&std::env::var("KEY_DIR").expect("KEY_DIR not set"));
    out_dir.push(&format!(
        "account-{}_balance-{}",
        account_tree_depth(),
        BALANCE_TREE_DEPTH
    ));
    out_dir
}

fn base_universal_setup_dir() -> Result<PathBuf, failure::Error> {
    let mut dir = PathBuf::new();
    // root is used by default for provers
    dir.push(&std::env::var("ZKSYNC_HOME").unwrap_or_else(|_| "/".to_owned()));
    dir.push("keys");
    dir.push("setup");
    failure::ensure!(dir.exists(), "Universal setup dir does not exits");
    Ok(dir)
}

/// Returns paths for universal setup in monomial form of the given power of two (range: 20-26). Checks if file exists
pub fn get_universal_setup_monomial_form_file_path(
    power_of_two: usize,
) -> Result<PathBuf, failure::Error> {
    failure::ensure!(
        (20..=26).contains(&power_of_two),
        "power of two is not in [20,26] range"
    );
    let setup_file_name = format!("setup_2^{}.key", power_of_two);
    let mut setup_file = base_universal_setup_dir()?;
    setup_file.push(&setup_file_name);
    failure::ensure!(
        setup_file.exists(),
        "Universal setup file {} does not exist",
        setup_file_name
    );
    Ok(setup_file)
}

/// Returns paths for universal setup in lagrange form of the given power of two (range: 20-26). Checks if file exists
pub fn get_universal_setup_lagrange_form_file_path(
    power_of_two: usize,
) -> Result<PathBuf, failure::Error> {
    failure::ensure!(
        (20..=26).contains(&power_of_two),
        "power of two is not in [20,26] range"
    );
    let setup_file_name = format!("setup_2^{}_lagrange.key", power_of_two);
    let mut setup_file = base_universal_setup_dir()?;
    setup_file.push(&setup_file_name);

    failure::ensure!(
        setup_file.exists(),
        "Universal setup file {} does not exist",
        setup_file_name
    );
    Ok(setup_file)
}

pub fn get_block_proof_key_and_vk_path(block_size: usize) -> (PathBuf, PathBuf) {
    let mut out_dir = get_keys_root_dir();
    out_dir.push(&format!("block-{}", block_size));

    let mut key_file = out_dir.clone();
    key_file.push(KEY_FILENAME);

    let mut get_vk_file = out_dir;
    get_vk_file.push(VERIFY_KEY_FILENAME);

    (key_file, get_vk_file)
}

pub fn get_exodus_proof_key_and_vk_path() -> (PathBuf, PathBuf) {
    let mut out_dir = get_keys_root_dir();
    out_dir.push("exodus_key");

    let mut key_file = out_dir.clone();
    key_file.push(EXIT_KEY_FILENAME);

    let mut get_vk_file = out_dir;
    get_vk_file.push(VERIFY_KEY_FILENAME);

    (key_file, get_vk_file)
}

pub fn get_exodus_verification_key_path() -> PathBuf {
    let mut key = get_keys_root_dir();
    key.push("verification_exit.key");
    key
}

pub fn get_block_verification_key_path(block_chunks: usize) -> PathBuf {
    let mut key = get_keys_root_dir();
    key.push(&format!("verification_block_{}.key", block_chunks));
    key
}

pub fn get_verifier_contract_key_path() -> PathBuf {
    let mut contract = get_keys_root_dir();
    contract.push("Verifier.sol");
    contract
}
