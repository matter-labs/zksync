use crate::fs_utils::get_recursive_verification_key_path;
use crate::serialization::{
    serialize_fe_for_ethereum, serialize_new_proof, AggregatedProofSerde, SingleProofSerde,
};
use crate::{get_universal_setup_monomial_form, PlonkVerificationKey};
use serde::export::Formatter;
use serde::{Deserialize, Serialize};
use std::fs::File;
use zksync_basic_types::U256;
use zksync_crypto::bellman::pairing::ff::{BitIterator, Field, PrimeField, PrimeFieldRepr};
use zksync_crypto::bellman::pairing::{CurveAffine, Engine as EngineTrait};
use zksync_crypto::bellman::plonk::better_better_cs::cs::Circuit as NewCircuit;
use zksync_crypto::bellman::plonk::better_better_cs::proof::Proof as NewProof;
use zksync_crypto::bellman::plonk::better_better_cs::{
    setup::VerificationKey as VkAggregate, verifier::verify,
};
use zksync_crypto::bellman::plonk::better_cs::{
    cs::PlonkCsWidth4WithNextStepParams,
    keys::{Proof, VerificationKey as SingleVk},
};
use zksync_crypto::bellman::worker::Worker;
use zksync_crypto::ff::ScalarEngine;
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
use zksync_crypto::franklin_crypto::bellman::plonk::commitments::transcript::keccak_transcript::RollingKeccakTranscript;
use zksync_crypto::params::{
    RECURSIVE_CIRCUIT_NUM_INPUTS, RECURSIVE_CIRCUIT_SIZES, RECURSIVE_CIRCUIT_VK_TREE_DEPTH,
};
use zksync_crypto::proof::EncodedAggregatedProof;
use zksync_crypto::recursive_aggregation_circuit::circuit::{
    create_recursive_circuit_setup, create_zksync_recursive_aggregate,
    proof_recursive_aggregate_for_zksync, RecursiveAggregationCircuitBn256,
};
use zksync_crypto::serialization::VecFrSerde;
use zksync_crypto::{Engine, Fr};
// use models::config_options::{get_env, parse_env, AvailableBlockSizesConfig};
// use models::primitives::serialize_fe_for_ethereum;
// use models::prover_utils::fs_utils::get_recursive_verification_key_path;
// use models::prover_utils::{
//     get_universal_setup_monomial_form, save_to_cache_universal_setup_monomial_form,
//     serialize_new_proof, EncodedProofPlonk,
// };
// use models::prover_utils::{PlonkVerificationKey, SetupForStepByStepProver};
// use std::fs::File;
// use std::sync::{mpsc, Mutex};
// use std::time::Duration;

pub type OldProofType = Proof<Engine, PlonkCsWidth4WithNextStepParams>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleProof(#[serde(with = "SingleProofSerde")] pub(crate) OldProofType);

impl Default for SingleProof {
    fn default() -> Self {
        SingleProof(OldProofType::empty())
    }
}

pub type NewProofType = NewProof<Engine, RecursiveAggregationCircuitBn256<'static>>;
#[derive(Serialize, Deserialize)]
pub struct AggregatedProof {
    #[serde(with = "AggregatedProofSerde")]
    pub(crate) proof: NewProofType,
    #[serde(with = "VecFrSerde")]
    pub(crate) individual_vk_inputs: Vec<Fr>,
    pub(crate) individual_vk_idxs: Vec<usize>,
    #[serde(with = "VecFrSerde")]
    pub(crate) aggr_limbs: Vec<Fr>,
}

impl Default for AggregatedProof {
    fn default() -> Self {
        AggregatedProof {
            proof: NewProofType::empty(),
            individual_vk_inputs: Vec::new(),
            individual_vk_idxs: Vec::new(),
            aggr_limbs: Vec::new(),
        }
    }
}

impl std::fmt::Debug for AggregatedProof {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "AggregatedProof")
    }
}

impl Clone for AggregatedProof {
    fn clone(&self) -> Self {
        let mut bytes = Vec::new();
        self.proof
            .write(&mut bytes)
            .expect("Failed to serialize aggregated proof");
        AggregatedProof {
            proof: NewProof::read(&*bytes).expect("Failed to deserialize aggregated proof"),
            individual_vk_inputs: self.individual_vk_inputs.clone(),
            individual_vk_idxs: self.individual_vk_idxs.clone(),
            aggr_limbs: self.aggr_limbs.clone(),
        }
    }
}

pub type Vk = SingleVk<Engine, PlonkCsWidth4WithNextStepParams>;

#[derive(Serialize, Deserialize)]
pub struct AggregatedProofData {
    pub(crate) proof: AggregatedProof,
    #[serde(with = "VecFrSerde")]
    pub(crate) aggregated_limbs: Vec<Fr>,
    #[serde(with = "VecFrSerde")]
    pub(crate) individual_inputs: Vec<Fr>,
    pub(crate) individual_idx: Vec<usize>,
}

#[derive(Clone)]
pub struct SingleProofData {
    pub proof: SingleProof,
    pub vk_idx: usize,
}

pub fn prepare_proof_data(
    available_chunks: &[usize],
    proofs: Vec<(SingleProof, usize)>,
) -> (Vec<Vk>, Vec<SingleProofData>) {
    let all_vks = available_chunks
        .iter()
        .map(|chunks| {
            PlonkVerificationKey::read_verification_key_for_main_circuit(*chunks)
                .unwrap()
                .0
        })
        .collect::<Vec<_>>();

    let mut single_proof_data = Vec::new();
    for (proof, block_size) in proofs {
        let (vk_idx, _) = available_chunks
            .iter()
            .enumerate()
            .find(|(idx, size)| **size == block_size)
            .expect("block size not found");

        single_proof_data.push(SingleProofData { proof, vk_idx });
    }
    (all_vks, single_proof_data)
}

pub fn gen_aggregate_proof(
    single_vks: Vec<Vk>,
    proofs: Vec<SingleProofData>,
    download_setup_network: bool,
) -> anyhow::Result<AggregatedProof> {
    // proofs: Vec<SingleProofData>,
    let mut individual_vk_inputs = Vec::new();
    let mut individual_vk_idxs = Vec::new();
    for p in &proofs {
        let individual_input = {
            anyhow::ensure!(
                p.proof.0.input_values.len() == 1,
                "Single should have one input"
            );
            p.proof.0.input_values[0]
        };
        individual_vk_inputs.push(individual_input);
        individual_vk_idxs.push(p.vk_idx);
    }

    let worker = Worker::new();

    let universal_setup = {
        let setup_power = RECURSIVE_CIRCUIT_SIZES
            .iter()
            .find_map(|(aggr_size, aggregate_setup_power)| {
                if *aggr_size == proofs.len() {
                    Some(*aggregate_setup_power)
                } else {
                    None
                }
            })
            .ok_or(anyhow::anyhow!("Recursive proof size not found"))?;
        get_universal_setup_monomial_form(setup_power, download_setup_network)?
    };
    let mut g2_bases = [<<Engine as EngineTrait>::G2Affine as CurveAffine>::zero(); 2];
    g2_bases.copy_from_slice(&universal_setup.g2_monomial_bases.as_ref()[..]);
    let (proofs, vk_indexes) = proofs.clone().into_iter().fold(
        (Vec::new(), Vec::new()),
        |(mut proofs, mut vk_idxs), SingleProofData { proof, vk_idx }| {
            proofs.push(proof.0);
            vk_idxs.push(vk_idx);
            (proofs, vk_idxs)
        },
    );
    let aggregate = create_zksync_recursive_aggregate(
        RECURSIVE_CIRCUIT_VK_TREE_DEPTH,
        RECURSIVE_CIRCUIT_NUM_INPUTS,
        &single_vks,
        &proofs,
        &vk_indexes,
        &g2_bases,
    )?;
    let aggr_limbs = aggregate.limbed_aggregated_g1_elements.clone();

    let setup = create_recursive_circuit_setup(
        proofs.len(),
        RECURSIVE_CIRCUIT_NUM_INPUTS,
        RECURSIVE_CIRCUIT_VK_TREE_DEPTH,
    )
    .expect("failed to create_recursive_circuit_vk_and_setup");

    let vk_for_recursive_circuit = VkAggregate::read(
        File::open(get_recursive_verification_key_path(proofs.len()))
            .expect("recursive verification key not found"),
    )
    .expect("recursive verification key read fail");

    let rec_aggr_proof = proof_recursive_aggregate_for_zksync(
        RECURSIVE_CIRCUIT_VK_TREE_DEPTH,
        RECURSIVE_CIRCUIT_NUM_INPUTS,
        &single_vks,
        &proofs,
        &vk_indexes,
        &vk_for_recursive_circuit,
        &setup,
        &universal_setup,
        true,
        &worker,
    )
    .expect("must create aggregate");
    // save_to_cache_universal_setup_monomial_form(setup_power, universal_setup);

    let is_valid = verify::<_, _, RollingKeccakTranscript<<Engine as ScalarEngine>::Fr>>(
        &vk_for_recursive_circuit,
        &rec_aggr_proof,
        None,
    )
    .expect("must perform verification");
    if !is_valid {
        return Err(anyhow::anyhow!("Recursive proof is invalid"));
    };

    Ok(AggregatedProof {
        proof: rec_aggr_proof,
        individual_vk_inputs,
        individual_vk_idxs,
        aggr_limbs,
    })
}

impl AggregatedProof {
    pub fn serialize_aggregated_proof(&self) -> EncodedAggregatedProof {
        let (inputs, proof) = serialize_new_proof(&self.proof);

        let subproof_limbs = self
            .aggr_limbs
            .iter()
            .map(serialize_fe_for_ethereum)
            .collect();
        let individual_vk_inputs = self
            .individual_vk_inputs
            .iter()
            .map(serialize_fe_for_ethereum)
            .collect();
        let individual_vk_idxs = self
            .individual_vk_idxs
            .iter()
            .cloned()
            .map(U256::from)
            .collect();

        EncodedAggregatedProof {
            aggregated_input: inputs[0],
            proof,
            subproof_limbs,
            individual_vk_inputs,
            individual_vk_idxs,
        }
    }
}
