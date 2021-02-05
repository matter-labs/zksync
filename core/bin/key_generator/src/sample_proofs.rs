//!

use zksync_circuit::witness::WitnessBuilder;
use zksync_config::ChainConfig;
use zksync_crypto::circuit::CircuitAccountTree;
use zksync_crypto::params::account_tree_depth;
use zksync_crypto::proof::{PrecomputedSampleProofs, SingleProof};
use zksync_prover_utils::aggregated_proofs::{gen_aggregate_proof, prepare_proof_data};
use zksync_prover_utils::fs_utils::get_precomputed_proofs_path;
use zksync_prover_utils::{PlonkVerificationKey, SetupForStepByStepProver};
use zksync_types::{Account, AccountId, BlockNumber};

fn generate_zksync_circuit_proofs(
    amount: usize,
    block_size: usize,
) -> anyhow::Result<Vec<(SingleProof, usize)>> {
    let mut proofs = Vec::new();
    for n in 0..amount {
        let zksync_circuit = {
            let block_number = BlockNumber(n as u32);

            let mut account_tree = CircuitAccountTree::new(account_tree_depth());
            account_tree.insert(0, Account::default().into());
            let mut witness_builder =
                WitnessBuilder::new(&mut account_tree, AccountId(0), block_number, 0);
            witness_builder.extend_pubdata_with_noops(block_size);
            witness_builder.collect_fees(&[]);
            witness_builder.calculate_pubdata_commitment();
            witness_builder.into_circuit_instance()
        };

        let setup = SetupForStepByStepProver::prepare_setup_for_step_by_step_prover(
            zksync_circuit.clone(),
            false,
        )?;

        let vk = PlonkVerificationKey::read_verification_key_for_main_circuit(block_size)?;

        let verified_proof =
            setup.gen_step_by_step_proof_using_prepared_setup(zksync_circuit, &vk)?;

        proofs.push((verified_proof, block_size));
    }
    Ok(proofs)
}

pub fn make_sample_proofs(config: ChainConfig) -> anyhow::Result<()> {
    let block_size = *config
        .circuit
        .supported_block_chunks_sizes
        .iter()
        .min()
        .ok_or_else(|| anyhow::anyhow!("Block sizes list should not be empty"))?;

    let max_aggregated_size = *config
        .circuit
        .supported_aggregated_proof_sizes
        .iter()
        .max()
        .ok_or_else(|| anyhow::anyhow!("Aggregated proof sizes should not be empty"))?;
    let single_proofs = generate_zksync_circuit_proofs(max_aggregated_size, block_size)?;

    let aggregated_proof = {
        let min_aggregated_size = *config
            .circuit
            .supported_aggregated_proof_sizes
            .iter()
            .min()
            .ok_or_else(|| anyhow::anyhow!("Aggregated proof sizes should not be empty"))?;
        let proofs_to_aggregate = single_proofs
            .clone()
            .into_iter()
            .take(min_aggregated_size)
            .collect();
        let (vks, proof_data) = prepare_proof_data(
            &config.circuit.supported_block_chunks_sizes,
            proofs_to_aggregate,
        );
        gen_aggregate_proof(
            vks,
            proof_data,
            &config
                .circuit
                .supported_aggregated_proof_sizes_with_setup_pow(),
            false,
        )?
    };

    let precomputed_proofs = PrecomputedSampleProofs {
        single_proofs,
        aggregated_proof,
    };

    let serialized = serde_json::to_vec_pretty(&precomputed_proofs)?;
    std::fs::write(get_precomputed_proofs_path(), &serialized)?;
    Ok(())
}
