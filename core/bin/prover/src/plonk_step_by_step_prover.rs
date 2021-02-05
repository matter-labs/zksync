// Built-in deps
use std::sync::Mutex;
// Workspace deps
use zksync_config::ChainConfig;
use zksync_crypto::proof::{AggregatedProof, PrecomputedSampleProofs, SingleProof};
use zksync_crypto::Engine;
use zksync_prover_utils::aggregated_proofs::{gen_aggregate_proof, prepare_proof_data};
use zksync_prover_utils::api::{JobRequestData, JobResultData};
use zksync_prover_utils::{PlonkVerificationKey, SetupForStepByStepProver};
use zksync_utils::parse_env;
// Local deps
use crate::{ProverConfig, ProverImpl};
use zksync_prover_utils::fs_utils::load_precomputed_proofs;

/// We prepare some data before making proof for each block size, so we cache it in case next block
/// would be of our size
struct PreparedComputations {
    block_size: usize,
    setup: SetupForStepByStepProver,
}

pub struct PlonkStepByStepProver {
    config: PlonkStepByStepProverConfig,
    prepared_computations: Mutex<Option<PreparedComputations>>,
    precomputed_sample_proofs: PrecomputedSampleProofs,
}

pub struct PlonkStepByStepProverConfig {
    pub all_block_sizes: Vec<usize>,
    pub block_sizes: Vec<usize>,
    pub download_setup_from_network: bool,
    pub aggregated_proof_sizes_with_setup_pow: Vec<(usize, u32)>,
}

impl ProverConfig for PlonkStepByStepProverConfig {
    fn from_env() -> Self {
        let env_config = ChainConfig::from_env();

        let aggregated_proof_sizes_with_setup_pow = env_config
            .circuit
            .supported_aggregated_proof_sizes_with_setup_pow();

        Self {
            download_setup_from_network: parse_env("MISC_PROVER_DOWNLOAD_SETUP"),
            all_block_sizes: env_config.circuit.supported_block_chunks_sizes,
            block_sizes: env_config.state_keeper.block_chunk_sizes,
            aggregated_proof_sizes_with_setup_pow,
        }
    }
}

impl PlonkStepByStepProver {
    fn create_single_block_proof(
        &self,
        witness: zksync_circuit::circuit::ZkSyncCircuit<'_, Engine>,
        block_size: usize,
    ) -> anyhow::Result<SingleProof> {
        // we do this way here so old precomp is dropped
        let valid_cached_precomp = {
            self.prepared_computations
                .lock()
                .unwrap()
                .take()
                .filter(|p| p.block_size == block_size)
        };
        let precomp = if let Some(precomp) = valid_cached_precomp {
            precomp
        } else {
            let setup = SetupForStepByStepProver::prepare_setup_for_step_by_step_prover(
                witness.clone(),
                self.config.download_setup_from_network,
            )?;
            PreparedComputations { block_size, setup }
        };

        let vk = PlonkVerificationKey::read_verification_key_for_main_circuit(block_size)?;
        let verified_proof = precomp
            .setup
            .gen_step_by_step_proof_using_prepared_setup(witness, &vk)?;

        *self.prepared_computations.lock().unwrap() = Some(precomp);

        Ok(verified_proof)
    }

    fn create_aggregated_block_proof(
        &self,
        proofs: Vec<(SingleProof, usize)>,
    ) -> anyhow::Result<AggregatedProof> {
        // drop setup cache
        {
            self.prepared_computations.lock().unwrap().take();
        }

        let proofs_to_pad = {
            let aggregate_size = self.config.aggregated_proof_sizes_with_setup_pow.iter().find(|(aggregate_size, _)| aggregate_size >= &proofs.len())
                .ok_or_else(|| {
                    anyhow::anyhow!("Failed to find aggregate proof size to fit all proofs, size: {:?}, proofs: {}", self.config.aggregated_proof_sizes_with_setup_pow, proofs.len())
                })?.0;
            aggregate_size
                .checked_sub(proofs.len())
                .expect("Aggregate size should be <= number of proofs")
        };

        if proofs_to_pad > 0 {
            vlog::info!(
                "Padding aggregated proofs. proofs: {}, proofs to pad: {}, aggregate_size: {}",
                proofs.len(),
                proofs_to_pad,
                proofs.len() + proofs_to_pad
            );
        }

        let padded_proofs = proofs
            .into_iter()
            .chain(
                self.precomputed_sample_proofs
                    .single_proofs
                    .iter()
                    .cloned()
                    .take(proofs_to_pad),
            )
            .collect();

        let (vks, proof_data) = prepare_proof_data(&self.config.all_block_sizes, padded_proofs);
        gen_aggregate_proof(
            vks,
            proof_data,
            &self.config.aggregated_proof_sizes_with_setup_pow,
            self.config.download_setup_from_network,
        )
    }
}

impl ProverImpl for PlonkStepByStepProver {
    type Config = PlonkStepByStepProverConfig;

    fn create_proof(&self, data: JobRequestData) -> Result<JobResultData, anyhow::Error> {
        let proof = match data {
            JobRequestData::AggregatedBlockProof(proofs_to_aggregate) => {
                let block_sizes = proofs_to_aggregate
                    .iter()
                    .map(|(_, s)| *s)
                    .collect::<Vec<_>>();

                let aggregate_proof = self.create_aggregated_block_proof(proofs_to_aggregate).map_err(|e| {
                    anyhow::format_err!("Failed to aggregate block proofs, num proofs: {}, block sizes: {:?}, err {}", block_sizes.len(), &block_sizes, e)
                })?;

                JobResultData::AggregatedBlockProof(aggregate_proof)
            }
            JobRequestData::BlockProof(zksync_circuit, block_size) => {
                let zksync_circuit = zksync_circuit.into_circuit();
                let proof = self
                    .create_single_block_proof(zksync_circuit, block_size)
                    .map_err(|e| {
                        anyhow::format_err!(
                            "Failed to create single block proof, block size: {}, err: {}",
                            block_size,
                            e
                        )
                    })?;

                JobResultData::BlockProof(proof)
            }
        };

        Ok(proof)
    }

    fn create_from_config(config: PlonkStepByStepProverConfig) -> Self {
        assert!(!config.block_sizes.is_empty());
        PlonkStepByStepProver {
            config,
            prepared_computations: Mutex::new(None),
            precomputed_sample_proofs: load_precomputed_proofs()
                .expect("Failed to load precomputed sample proofs"),
        }
    }
}
