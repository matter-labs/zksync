use crate::franklin_crypto::bellman::Circuit;
use crate::node::U256;
use crate::node::{Engine, Fr};
use crate::params::{account_tree_depth, BALANCE_TREE_DEPTH};
use crate::primitives::{serialize_fe_for_ethereum, serialize_g1_for_ethereum};
use crypto_exports::bellman::kate_commitment::{Crs, CrsForLagrangeForm, CrsForMonomialForm};
use crypto_exports::bellman::plonk::better_cs::{
    adaptor::TranspilationVariant, cs::PlonkCsWidth4WithNextStepParams, keys::Proof,
    keys::SetupPolynomials, keys::VerificationKey,
};
use crypto_exports::bellman::plonk::commitments::transcript::keccak_transcript::RollingKeccakTranscript;
use crypto_exports::bellman::plonk::{prove_by_steps, setup, transpile, verify};
use failure::format_err;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

pub struct PlonkVerificationKey(VerificationKey<Engine, PlonkCsWidth4WithNextStepParams>);

impl PlonkVerificationKey {
    pub fn read_verification_key_for_main_circuit(
        block_chunks: usize,
    ) -> Result<Self, failure::Error> {
        Ok(Self(VerificationKey::read(File::open(
            get_block_verification_key_path(block_chunks),
        )?)?))
    }

    pub fn read_verification_key_for_exit_circuit() -> Result<Self, failure::Error> {
        Ok(Self(VerificationKey::read(File::open(
            get_exodus_verification_key_path(),
        )?)?))
    }
}

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

fn get_universal_setup_file_buff_reader(
    setup_file_name: &str,
) -> Result<BufReader<File>, failure::Error> {
    let setup_file = {
        let mut path = base_universal_setup_dir()?;
        path.push(&setup_file_name);
        File::open(path).map_err(|e| {
            format_err!(
                "Failed to open universal setup file {}, err: {}",
                setup_file_name,
                e
            )
        })?
    };
    Ok(BufReader::with_capacity(1 << 29, setup_file))
}

/// Returns universal setup in the monomial form of the given power of two (range: 20-26). Checks if file exists
pub fn get_universal_setup_monomial_form(
    power_of_two: u32,
) -> Result<Crs<Engine, CrsForMonomialForm>, failure::Error> {
    failure::ensure!(
        (20..=26).contains(&power_of_two),
        "power of two is not in [20,26] range"
    );
    let setup_file_name = format!("setup_2^{}.key", power_of_two);
    let mut buf_reader = get_universal_setup_file_buff_reader(&setup_file_name)?;
    Ok(Crs::<Engine, CrsForMonomialForm>::read(&mut buf_reader)
        .map_err(|e| format_err!("Failed to read Crs from setup file: {}", e))?)
}

/// Returns universal setup in lagrange form of the given power of two (range: 20-26). Checks if file exists
pub fn get_universal_setup_lagrange_form(
    power_of_two: u32,
) -> Result<Crs<Engine, CrsForLagrangeForm>, failure::Error> {
    failure::ensure!(
        (20..=26).contains(&power_of_two),
        "power of two is not in [20,26] range"
    );
    let setup_file_name = format!("setup_2^{}_lagrange.key", power_of_two);
    let mut buf_reader = get_universal_setup_file_buff_reader(&setup_file_name)?;
    Ok(Crs::<Engine, CrsForLagrangeForm>::read(&mut buf_reader)
        .map_err(|e| format_err!("Failed to read Crs from setup file: {}", e))?)
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
    contract.push("KeysWithPlonkVerifier.sol");
    contract
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct EncodedProofPlonk {
    pub inputs: Vec<U256>,
    pub proof: Vec<U256>,
}

pub struct SetupForStepByStepProver {
    setup_polynomials: SetupPolynomials<Engine, PlonkCsWidth4WithNextStepParams>,
    hints: Vec<(usize, TranspilationVariant)>,
    key_monomial_form: Crs<Engine, CrsForMonomialForm>,
}

impl SetupForStepByStepProver {
    pub fn prepare_setup_for_step_by_step_prover<C: Circuit<Engine> + Clone>(
        circuit: C,
    ) -> Result<Self, failure::Error> {
        let hints = transpile(circuit.clone())?;
        let setup_polynomials = setup(circuit, &hints)?;
        let size_log2 = setup_polynomials.n.next_power_of_two().trailing_zeros();
        let size_log2 = std::cmp::max(size_log2, 20); // for exit circuit
        let key_monomial_form = get_universal_setup_monomial_form(size_log2)?;
        Ok(SetupForStepByStepProver {
            setup_polynomials,
            hints,
            key_monomial_form,
        })
    }

    pub fn gen_step_by_step_proof_using_prepared_setup<C: Circuit<Engine> + Clone>(
        &self,
        circuit: C,
        vk: &PlonkVerificationKey,
    ) -> Result<EncodedProofPlonk, failure::Error> {
        let proof = prove_by_steps::<_, _, RollingKeccakTranscript<Fr>>(
            circuit,
            &self.hints,
            &self.setup_polynomials,
            None,
            &self.key_monomial_form,
        )?;

        let valid = verify::<_, RollingKeccakTranscript<Fr>>(&proof, &vk.0)?;
        failure::ensure!(valid, "proof for block is invalid");
        Ok(serialize_proof(&proof))
    }
}

/// Generates proof for exit given circuit using step-by-step algorithm.
pub fn gen_verified_proof_for_exit_circuit<C: Circuit<Engine> + Clone>(
    circuit: C,
) -> Result<EncodedProofPlonk, failure::Error> {
    let vk = VerificationKey::read(File::open(get_exodus_verification_key_path())?)?;

    info!("Proof for circuit started");

    let hints = transpile(circuit.clone())?;
    let setup = setup(circuit.clone(), &hints)?;
    let size_log2 = setup.n.next_power_of_two().trailing_zeros();

    let size_log2 = std::cmp::max(size_log2, 20); // for exit circuit
    let key_monomial_form = get_universal_setup_monomial_form(size_log2)?;

    let proof = prove_by_steps::<_, _, RollingKeccakTranscript<Fr>>(
        circuit,
        &hints,
        &setup,
        None,
        &key_monomial_form,
    )?;

    let valid = verify::<_, RollingKeccakTranscript<Fr>>(&proof, &vk)?;
    failure::ensure!(valid, "proof for exit is invalid");

    info!("Proof for circuit successful");
    Ok(serialize_proof(&proof))
}

pub fn serialize_proof(
    proof: &Proof<Engine, PlonkCsWidth4WithNextStepParams>,
) -> EncodedProofPlonk {
    let mut inputs = vec![];
    for input in proof.input_values.iter() {
        let ser = serialize_fe_for_ethereum(input);
        inputs.push(ser);
    }
    let mut serialized_proof = vec![];

    for c in proof.wire_commitments.iter() {
        let (x, y) = serialize_g1_for_ethereum(c);
        serialized_proof.push(x);
        serialized_proof.push(y);
    }

    let (x, y) = serialize_g1_for_ethereum(&proof.grand_product_commitment);
    serialized_proof.push(x);
    serialized_proof.push(y);

    for c in proof.quotient_poly_commitments.iter() {
        let (x, y) = serialize_g1_for_ethereum(c);
        serialized_proof.push(x);
        serialized_proof.push(y);
    }

    for c in proof.wire_values_at_z.iter() {
        serialized_proof.push(serialize_fe_for_ethereum(c));
    }

    for c in proof.wire_values_at_z_omega.iter() {
        serialized_proof.push(serialize_fe_for_ethereum(c));
    }

    serialized_proof.push(serialize_fe_for_ethereum(&proof.grand_product_at_z_omega));
    serialized_proof.push(serialize_fe_for_ethereum(&proof.quotient_polynomial_at_z));
    serialized_proof.push(serialize_fe_for_ethereum(
        &proof.linearization_polynomial_at_z,
    ));

    for c in proof.permutation_polynomials_at_z.iter() {
        serialized_proof.push(serialize_fe_for_ethereum(c));
    }

    let (x, y) = serialize_g1_for_ethereum(&proof.opening_at_z_proof);
    serialized_proof.push(x);
    serialized_proof.push(y);

    let (x, y) = serialize_g1_for_ethereum(&proof.opening_at_z_omega_proof);
    serialized_proof.push(x);
    serialized_proof.push(y);

    EncodedProofPlonk {
        inputs,
        proof: serialized_proof,
    }
}
