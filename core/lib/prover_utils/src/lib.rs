use crate::fs_utils::{get_block_verification_key_path, get_exodus_verification_key_path};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::fs::File;
use std::sync::{Arc, Mutex};
use zksync_crypto::bellman::kate_commitment::{Crs, CrsForMonomialForm};
use zksync_crypto::bellman::plonk::better_cs::{
    adaptor::TranspilationVariant, cs::PlonkCsWidth4WithNextStepParams, keys::SetupPolynomials,
    keys::VerificationKey, verifier::verify,
};
use zksync_crypto::bellman::plonk::{
    commitments::transcript::keccak_transcript::RollingKeccakTranscript, prove_by_steps, setup,
    transpile,
};
use zksync_crypto::franklin_crypto::bellman::Circuit;
use zksync_crypto::franklin_crypto::plonk::circuit::bigint::field::RnsParameters;
use zksync_crypto::franklin_crypto::rescue::bn256::Bn256RescueParams;
use zksync_crypto::franklin_crypto::rescue::rescue_transcript::RescueTranscriptForRNS;
use zksync_crypto::pairing::Engine as EngineTrait;
use zksync_crypto::params::RECURSIVE_CIRCUIT_VK_TREE_DEPTH;
use zksync_crypto::proof::SingleProof;
use zksync_crypto::recursive_aggregation_circuit::circuit::create_vks_tree;
use zksync_crypto::{Engine, Fr};

pub mod aggregated_proofs;
pub mod api;
pub mod exit_proof;
pub mod fs_utils;
pub mod network_utils;

pub const SETUP_MIN_POW2: u32 = 20;
pub const SETUP_MAX_POW2: u32 = 26;

pub struct PlonkVerificationKey(pub VerificationKey<Engine, PlonkCsWidth4WithNextStepParams>);

impl PlonkVerificationKey {
    pub fn read_verification_key_for_main_circuit(
        block_chunks: usize,
    ) -> Result<Self, anyhow::Error> {
        let verification_key =
            VerificationKey::read(File::open(get_block_verification_key_path(block_chunks))?)?;
        Ok(Self(verification_key))
    }

    pub fn read_verification_key_for_exit_circuit() -> Result<Self, anyhow::Error> {
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
    ) -> Result<Self, anyhow::Error> {
        let hints = transpile(circuit.clone())?;
        let setup_polynomials = setup(circuit, &hints)?;
        let size = setup_polynomials.n.next_power_of_two().trailing_zeros();
        let setup_power_of_two = std::cmp::max(size, SETUP_MIN_POW2); // for exit circuit
        let key_monomial_form = Some(get_universal_setup_monomial_form(
            setup_power_of_two,
            download_setup_file,
        )?);
        Ok(SetupForStepByStepProver {
            setup_polynomials,
            hints,
            setup_power_of_two,
            key_monomial_form,
        })
    }

    pub fn gen_step_by_step_proof_using_prepared_setup<C: Circuit<Engine> + Clone>(
        &self,
        circuit: C,
        vk: &PlonkVerificationKey,
    ) -> Result<SingleProof, anyhow::Error> {
        let rns_params =
            RnsParameters::<Engine, <Engine as EngineTrait>::Fq>::new_for_field(68, 110, 4);
        let rescue_params = Bn256RescueParams::new_checked_2_into_1();

        let transcript_params = (&rescue_params, &rns_params);
        let proof = prove_by_steps::<_, _, RescueTranscriptForRNS<Engine>>(
            circuit,
            &self.hints,
            &self.setup_polynomials,
            None,
            self.key_monomial_form
                .as_ref()
                .expect("Setup should have universal setup struct"),
            Some(transcript_params),
        )?;

        let valid =
            verify::<_, _, RescueTranscriptForRNS<Engine>>(&proof, &vk.0, Some(transcript_params))?;
        anyhow::ensure!(valid, "proof for block is invalid");
        Ok(proof.into())
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
) -> Result<SingleProof, anyhow::Error> {
    let vk = VerificationKey::read(File::open(get_exodus_verification_key_path())?)?;

    vlog::info!("Proof for circuit started");

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

    let valid = verify::<_, _, RollingKeccakTranscript<Fr>>(&proof, &vk, None)?;
    anyhow::ensure!(valid, "proof for exit is invalid");

    vlog::info!("Proof for circuit successful");
    Ok(proof.into())
}

/// Reads universal setup from disk or downloads from network.
pub fn get_universal_setup_monomial_form(
    power_of_two: u32,
    download_from_network: bool,
) -> Result<Crs<Engine, CrsForMonomialForm>, anyhow::Error> {
    if let Some(cached_setup) = UNIVERSAL_SETUP_CACHE.take_setup_struct(power_of_two) {
        Ok(cached_setup)
    } else if download_from_network {
        network_utils::get_universal_setup_monomial_form(power_of_two)
    } else {
        fs_utils::get_universal_setup_monomial_form(power_of_two)
    }
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
