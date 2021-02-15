use crate::fs_utils::get_recursive_verification_key_path;
use crate::{get_universal_setup_monomial_form, PlonkVerificationKey};
use std::fs::File;
use zksync_crypto::bellman::pairing::{CurveAffine, Engine as EngineTrait};
use zksync_crypto::bellman::plonk::better_better_cs::{
    setup::VerificationKey as VkAggregate, verifier::verify,
};
use zksync_crypto::bellman::worker::Worker;
use zksync_crypto::ff::ScalarEngine;
use zksync_crypto::franklin_crypto::bellman::plonk::commitments::transcript::keccak_transcript::RollingKeccakTranscript;
use zksync_crypto::params::{RECURSIVE_CIRCUIT_NUM_INPUTS, RECURSIVE_CIRCUIT_VK_TREE_DEPTH};
use zksync_crypto::proof::{AggregatedProof, SingleProof, Vk};
use zksync_crypto::recursive_aggregation_circuit::circuit::{
    create_recursive_circuit_setup, create_zksync_recursive_aggregate,
    proof_recursive_aggregate_for_zksync,
};
use zksync_crypto::Engine;
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
            .find(|(_, size)| **size == block_size)
            .expect("block size not found");

        single_proof_data.push(SingleProofData { proof, vk_idx });
    }
    (all_vks, single_proof_data)
}

pub fn gen_aggregate_proof(
    single_vks: Vec<Vk>,
    proofs: Vec<SingleProofData>,
    available_aggregated_proof_sizes: &[(usize, u32)],
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
        let setup_power = available_aggregated_proof_sizes
            .iter()
            .find_map(|(aggr_size, aggregate_setup_power)| {
                if *aggr_size == proofs.len() {
                    Some(*aggregate_setup_power)
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Recursive proof size not found"))?;
        get_universal_setup_monomial_form(setup_power, download_setup_network)?
    };
    let mut g2_bases = [<<Engine as EngineTrait>::G2Affine as CurveAffine>::zero(); 2];
    g2_bases.copy_from_slice(&universal_setup.g2_monomial_bases.as_ref()[..]);
    let (proofs, vk_indexes) = proofs.into_iter().fold(
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
    let aggr_limbs = aggregate.limbed_aggregated_g1_elements;

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
