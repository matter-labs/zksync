use crate::franklin_crypto::bellman::Circuit;
use crate::node::U256;
use crate::node::{Engine, Fr};
use crate::params::RECURSIVE_CIRCUIT_VK_TREE_DEPTH;
use crate::primitives::{serialize_fe_for_ethereum, serialize_g1_for_ethereum};
use crate::prover_utils::fs_utils::{
    get_block_verification_key_path, get_exodus_verification_key_path,
};
use crypto_exports::bellman::kate_commitment::{Crs, CrsForMonomialForm};
use crypto_exports::bellman::plonk::better_better_cs::cs::Circuit as NewCircuit;
use crypto_exports::bellman::plonk::better_better_cs::proof::Proof as NewProof;
use crypto_exports::bellman::plonk::better_cs::{
    adaptor::TranspilationVariant, cs::PlonkCsWidth4WithNextStepParams, keys::Proof,
    keys::SetupPolynomials, keys::VerificationKey, verifier::verify,
};
use crypto_exports::bellman::plonk::commitments::transcript::keccak_transcript::RollingKeccakTranscript;
use crypto_exports::bellman::plonk::{prove_by_steps, setup, transpile};
use crypto_exports::franklin_crypto::plonk::circuit::bigint::field::RnsParameters;
use crypto_exports::franklin_crypto::rescue::bn256::Bn256RescueParams;
use crypto_exports::franklin_crypto::rescue::rescue_transcript::RescueTranscriptForRNS;
use crypto_exports::pairing::Engine as EngineTrait;
use crypto_exports::recursive_aggregation_circuit::circuit::create_vks_tree;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::fs::File;
use std::sync::{Arc, Mutex};

pub mod fs_utils;
pub mod network_utils;

pub const SETUP_MIN_POW2: u32 = 20;
pub const SETUP_MAX_POW2: u32 = 26;

pub struct PlonkVerificationKey(pub VerificationKey<Engine, PlonkCsWidth4WithNextStepParams>);

impl PlonkVerificationKey {
    pub fn read_verification_key_for_main_circuit(
        block_chunks: usize,
    ) -> Result<Self, failure::Error> {
        let verification_key =
            VerificationKey::read(File::open(get_block_verification_key_path(block_chunks))?)?;
        Ok(Self(verification_key))
    }

    pub fn read_verification_key_for_exit_circuit() -> Result<Self, failure::Error> {
        let verification_key =
            VerificationKey::read(File::open(get_exodus_verification_key_path())?)?;
        Ok(Self(verification_key))
    }

    pub fn get_vk_tree_root_hash(blocks_chunks: &[usize]) -> Fr {
        let block_vks = blocks_chunks
            .iter()
            .map(|block_chunks| {
                PlonkVerificationKey::read_verification_key_for_main_circuit(*block_chunks)
                    .expect("Failed to get block vk")
                    .0
            })
            .collect::<Vec<_>>();
        let (_, (vk_tree, _)) = create_vks_tree(&block_vks, RECURSIVE_CIRCUIT_VK_TREE_DEPTH)
            .expect("Failed to create vk tree");
        vk_tree.get_commitment()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EncodedProofPlonk {
    pub inputs: Vec<U256>,
    pub proof: Vec<U256>,
    pub proof_binary: Vec<u8>,
    pub subproof_limbs: Vec<U256>,
}

impl Default for EncodedProofPlonk {
    fn default() -> Self {
        Self {
            inputs: vec![U256::default(); 1],
            proof: vec![U256::default(); 33],
            proof_binary: Vec::new(),
            subproof_limbs: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EncodedMultiblockProofPlonk {
    pub proven_blocks: u32,
    pub proof: EncodedProofPlonk,
}

pub struct SetupForStepByStepProver {
    setup_polynomials: SetupPolynomials<Engine, PlonkCsWidth4WithNextStepParams>,
    hints: Vec<(usize, TranspilationVariant)>,
    setup_power_of_two: u32,
    key_monomial_form: Option<Crs<Engine, CrsForMonomialForm>>,
}

impl SetupForStepByStepProver {
    pub fn prepare_setup_for_step_by_step_prover<C: Circuit<Engine> + Clone>(
        circuit: C,
        download_setup_file: bool,
    ) -> Result<Self, failure::Error> {
        let hints = transpile(circuit.clone())?;
        let setup_polynomials = setup(circuit, &hints)?;
        let size = setup_polynomials.n.next_power_of_two().trailing_zeros();
        let setup_power_of_two = std::cmp::max(size, SETUP_MIN_POW2); // for exit circuit
        let key_monomial_form = Some(get_universal_setup_monomial_form(
            setup_power_of_two,
            download_setup_file,
        )?);
        Ok(SetupForStepByStepProver {
            setup_power_of_two,
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
        let rns_params =
            RnsParameters::<Engine, <Engine as EngineTrait>::Fq>::new_for_field(68, 110, 4);
        let rescue_params = Bn256RescueParams::new_checked_2_into_1();

        let transcript_params = (&rescue_params, &rns_params);
        let may_be_proof = prove_by_steps::<_, _, RescueTranscriptForRNS<Engine>>(
            circuit.clone(),
            &self.hints,
            &self.setup_polynomials,
            None,
            self.key_monomial_form
                .as_ref()
                .expect("Setup should have universal setup struct"),
            Some(transcript_params),
        );
        if let Some(error) = may_be_proof.as_ref().err() {
            // make test CS and find what is a problem

            let mut cs = crypto_exports::franklin_crypto::circuit::test::TestConstraintSystem::<Engine>::new();
            let err = circuit.synthesize(&mut cs);
            if let Some(err) = err.err() {
                return Err(failure::format_err!("Debugging test CS failed to synthesize for failed proof, error: {}", err));
            }

            if let Some(potentially_failed_constraint) = cs.which_is_unsatisfied() {
                return Err(failure::format_err!("Error: {}. Test constraint system found failed constraint: {}", crypto_exports::bellman::SynthesisError::Unsatisfiable, potentially_failed_constraint));
            }
            
            return Err(failure::format_err!("Proof was not generated, error: {}", error));
        }

        let proof = may_be_proof.expect("proof is made");
        let valid =
            verify::<_, _, RescueTranscriptForRNS<Engine>>(&proof, &vk.0, Some(transcript_params))?;
        failure::ensure!(valid, "proof for block is invalid");
        Ok(serialize_proof(&proof))
    }
}

impl Drop for SetupForStepByStepProver {
    fn drop(&mut self) {
        let setup = self
            .key_monomial_form
            .take()
            .expect("Setup should have universal setup struct");
        UNIVERSAL_SETUP_CACHE.put_setup_struct(self.setup_power_of_two, setup);
    }
}

/// Generates proof for exit given circuit using step-by-step algorithm.
pub fn gen_verified_proof_for_exit_circuit<C: Circuit<Engine> + Clone>(
    circuit: C,
) -> Result<EncodedProofPlonk, failure::Error> {
    // let vk = VerificationKey::read(File::open(get_exodus_verification_key_path())?)?;

    info!("Proof for circuit started");

    let hints = transpile(circuit.clone())?;
    let setup = setup(circuit.clone(), &hints)?;
    let size_log2 = setup.n.next_power_of_two().trailing_zeros();

    let size_log2 = std::cmp::max(size_log2, SETUP_MIN_POW2); // for exit circuit
    let key_monomial_form = get_universal_setup_monomial_form(size_log2, false)?;

    let proof = prove_by_steps::<_, _, RollingKeccakTranscript<Fr>>(
        circuit,
        &hints,
        &setup,
        None,
        &key_monomial_form,
        None,
    )?;

    // let valid = verify::<_, RollingKeccakTranscript<Fr>>(&proof, &vk)?;
    // failure::ensure!(valid, "proof for exit is invalid");

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

    let mut proof_binary = Vec::new();
    proof
        .write(&mut proof_binary)
        .expect("old proof serialize fail");
    EncodedProofPlonk {
        inputs,
        proof: serialized_proof,
        proof_binary,
        subproof_limbs: Vec::new(),
    }
}

pub fn serialize_new_proof<C: NewCircuit<Engine>>(
    proof: &NewProof<Engine, C>,
) -> (Vec<U256>, Vec<U256>) {
    let mut inputs = vec![];
    for input in proof.inputs.iter() {
        inputs.push(serialize_fe_for_ethereum(&input));
    }
    let mut serialized_proof = vec![];

    for c in proof.state_polys_commitments.iter() {
        let (x, y) = serialize_g1_for_ethereum(&c);
        serialized_proof.push(x);
        serialized_proof.push(y);
    }

    let (x, y) = serialize_g1_for_ethereum(&proof.copy_permutation_grand_product_commitment);
    serialized_proof.push(x);
    serialized_proof.push(y);

    for c in proof.quotient_poly_parts_commitments.iter() {
        let (x, y) = serialize_g1_for_ethereum(&c);
        serialized_proof.push(x);
        serialized_proof.push(y);
    }

    for c in proof.state_polys_openings_at_z.iter() {
        serialized_proof.push(serialize_fe_for_ethereum(&c));
    }

    for (_, _, c) in proof.state_polys_openings_at_dilations.iter() {
        serialized_proof.push(serialize_fe_for_ethereum(&c));
    }

    assert_eq!(proof.gate_setup_openings_at_z.len(), 0);

    for (_, c) in proof.gate_selectors_openings_at_z.iter() {
        serialized_proof.push(serialize_fe_for_ethereum(&c));
    }

    for c in proof.copy_permutation_polys_openings_at_z.iter() {
        serialized_proof.push(serialize_fe_for_ethereum(&c));
    }

    serialized_proof.push(serialize_fe_for_ethereum(
        &proof.copy_permutation_grand_product_opening_at_z_omega,
    ));
    serialized_proof.push(serialize_fe_for_ethereum(&proof.quotient_poly_opening_at_z));
    serialized_proof.push(serialize_fe_for_ethereum(
        &proof.linearization_poly_opening_at_z,
    ));

    let (x, y) = serialize_g1_for_ethereum(&proof.opening_proof_at_z);
    serialized_proof.push(x);
    serialized_proof.push(y);

    let (x, y) = serialize_g1_for_ethereum(&proof.opening_proof_at_z_omega);
    serialized_proof.push(x);
    serialized_proof.push(y);

    (inputs, serialized_proof)
}

/// Reads universal setup from disk or downloads from network.
pub fn get_universal_setup_monomial_form(
    power_of_two: u32,
    download_from_network: bool,
) -> Result<Crs<Engine, CrsForMonomialForm>, failure::Error> {
    if let Some(cached_setup) = UNIVERSAL_SETUP_CACHE.take_setup_struct(power_of_two) {
        Ok(cached_setup)
    } else if download_from_network {
        network_utils::get_universal_setup_monomial_form(power_of_two)
    } else {
        fs_utils::get_universal_setup_monomial_form(power_of_two)
    }
}

pub fn save_to_cache_universal_setup_monomial_form(
    power_of_two: u32,
    setup: Crs<Engine, CrsForMonomialForm>,
) {
    UNIVERSAL_SETUP_CACHE.put_setup_struct(power_of_two, setup);
}

/// Plonk prover may need to change keys on the fly to prove block of the smaller size
/// cache is used to avoid downloading/loading from disk same files over and over again.
///
/// Note: Keeping all the key files at the same time in memory is not a huge overhead
/// (around 4GB, compared to 135GB that are used to generate proof)
struct UniversalSetupCache {
    data: Arc<Mutex<HashMap<u32, Crs<Engine, CrsForMonomialForm>>>>,
}

impl UniversalSetupCache {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn take_setup_struct(&self, setup_power: u32) -> Option<Crs<Engine, CrsForMonomialForm>> {
        self.data
            .lock()
            .expect("SetupPolynomialsCache lock")
            .remove(&setup_power)
    }

    pub fn put_setup_struct(&self, setup_power: u32, setup: Crs<Engine, CrsForMonomialForm>) {
        self.data
            .lock()
            .expect("SetupPolynomialsCache lock")
            .insert(setup_power, setup);
    }
}

lazy_static! {
    static ref UNIVERSAL_SETUP_CACHE: UniversalSetupCache = UniversalSetupCache::new();
}
