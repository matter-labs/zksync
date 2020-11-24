use crate::fs_utils::get_recursive_verification_key_path;
use crate::{get_universal_setup_monomial_form, PlonkVerificationKey};
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
    proof_recursive_aggregate_for_zksync,
};
use zksync_crypto::Engine;
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

pub type SingleProof = Proof<Engine, PlonkCsWidth4WithNextStepParams>;
pub type Vk = SingleVk<Engine, PlonkCsWidth4WithNextStepParams>;

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
) -> anyhow::Result<EncodedAggregatedProof> {
    // proofs: Vec<SingleProofData>,
    let mut individual_vk_inputs = Vec::new();
    let mut individual_vk_idxs = Vec::new();
    for p in &proofs {
        individual_vk_inputs.push(serialize_fe_for_ethereum(&p.proof.input_values[0]));
        individual_vk_idxs.push(U256::from(p.vk_idx));
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
            proofs.push(proof);
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
    let aggr_limbs = aggregate
        .limbed_aggregated_g1_elements
        .into_iter()
        .map(|l| serialize_fe_for_ethereum(&l))
        .collect::<Vec<_>>();

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
    let (inputs, proof) = serialize_new_proof(&rec_aggr_proof);

    Ok(EncodedAggregatedProof {
        aggregated_input: inputs[0],
        proof,
        subproof_limbs: aggr_limbs,
        individual_vk_inputs,
        individual_vk_idxs,
    })
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

pub fn serialize_fe_for_ethereum(field_element: &<Bn256 as ScalarEngine>::Fr) -> U256 {
    let mut be_bytes = [0u8; 32];
    field_element
        .into_repr()
        .write_be(&mut be_bytes[..])
        .expect("get new root BE bytes");
    U256::from_big_endian(&be_bytes[..])
}

pub fn serialize_g1_for_ethereum(point: &<Bn256 as EngineTrait>::G1Affine) -> (U256, U256) {
    if point.is_zero() {
        return (U256::zero(), U256::zero());
    }
    let uncompressed = point.into_uncompressed();

    let uncompressed_slice = uncompressed.as_ref();

    // bellman serializes points as big endian and in the form x, y
    // ethereum expects the same order in memory
    let x = U256::from_big_endian(&uncompressed_slice[0..32]);
    let y = U256::from_big_endian(&uncompressed_slice[32..64]);

    (x, y)
}

pub fn serialize_g2_for_ethereum(
    point: &<Bn256 as EngineTrait>::G2Affine,
) -> ((U256, U256), (U256, U256)) {
    let uncompressed = point.into_uncompressed();

    let uncompressed_slice = uncompressed.as_ref();

    // bellman serializes points as big endian and in the form x1*u, x0, y1*u, y0
    // ethereum expects the same order in memory
    let x_1 = U256::from_big_endian(&uncompressed_slice[0..32]);
    let x_0 = U256::from_big_endian(&uncompressed_slice[32..64]);
    let y_1 = U256::from_big_endian(&uncompressed_slice[64..96]);
    let y_0 = U256::from_big_endian(&uncompressed_slice[96..128]);

    ((x_1, x_0), (y_1, y_0))
}
