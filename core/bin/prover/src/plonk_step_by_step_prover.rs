// Built-in deps
use std::sync::Mutex;
// Workspace deps
use zksync_config::AvailableBlockSizesConfig;
use zksync_crypto::proof::{AggregatedProof, SingleProof};
use zksync_crypto::Engine;
use zksync_prover_utils::aggregated_proofs::{gen_aggregate_proof, prepare_proof_data};
use zksync_prover_utils::api::{JobRequestData, JobResultData};
use zksync_prover_utils::{PlonkVerificationKey, SetupForStepByStepProver};
use zksync_utils::{parse_env, parse_env_to_collection};
// Local deps
use crate::{ProverConfig, ProverImpl};

/// We prepare some data before making proof for each block size, so we cache it in case next block
/// would be of our size
struct PreparedComputations {
    block_size: usize,
    setup: SetupForStepByStepProver,
}

pub struct PlonkStepByStepProver {
    config: PlonkStepByStepProverConfig,
    prepared_computations: Mutex<Option<PreparedComputations>>,
}

pub struct PlonkStepByStepProverConfig {
    pub all_block_sizes: Vec<usize>,
    pub block_sizes: Vec<usize>,
    pub download_setup_from_network: bool,
}

impl ProverConfig for PlonkStepByStepProverConfig {
    fn from_env() -> Self {
        Self {
            all_block_sizes: parse_env_to_collection("SUPPORTED_BLOCK_CHUNKS_SIZES"),
            block_sizes: parse_env_to_collection("BLOCK_CHUNK_SIZES"),
            download_setup_from_network: parse_env("PROVER_DOWNLOAD_SETUP"),
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
        let (vks, proof_data) = prepare_proof_data(&self.config.all_block_sizes, proofs);

        let aggregated_proof_sizes_with_setup_pow =
            AvailableBlockSizesConfig::from_env().aggregated_proof_sizes_with_setup_pow();
        gen_aggregate_proof(
            vks,
            proof_data,
            &aggregated_proof_sizes_with_setup_pow,
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
        }
    }
}
