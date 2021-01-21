use crate::{ProverConfig, ProverImpl};
use anyhow::Error;
use zksync_config::ZkSyncConfig;
use zksync_crypto::proof::PrecomputedSampleProofs;
use zksync_prover_utils::api::{JobRequestData, JobResultData};
use zksync_prover_utils::fs_utils::load_precomputed_proofs;

#[derive(Debug)]
pub struct DummyProverConfig {
    pub block_sizes: Vec<usize>,
}

impl ProverConfig for DummyProverConfig {
    fn from_env() -> Self {
        let env_config = ZkSyncConfig::from_env();

        Self {
            block_sizes: env_config.chain.state_keeper.block_chunk_sizes,
        }
    }
}

#[derive(Debug)]
pub struct DummyProver {
    config: DummyProverConfig,
    precomputed_proofs: PrecomputedSampleProofs,
}

impl ProverImpl for DummyProver {
    type Config = DummyProverConfig;

    fn create_from_config(config: Self::Config) -> Self {
        Self {
            config,
            precomputed_proofs: load_precomputed_proofs()
                .expect("Failed to load precomputed proofs"),
        }
    }

    fn create_proof(&self, data: JobRequestData) -> Result<JobResultData, Error> {
        let empty_proof = match data {
            JobRequestData::AggregatedBlockProof(single_proofs) => {
                let mut aggregated_proof = self.precomputed_proofs.aggregated_proof.clone();
                aggregated_proof.individual_vk_inputs = Vec::new();
                for (single_proof, _) in single_proofs {
                    aggregated_proof
                        .individual_vk_inputs
                        .push(single_proof.0.input_values[0]);
                    aggregated_proof.individual_vk_idxs.push(0);
                }

                JobResultData::AggregatedBlockProof(aggregated_proof)
            }
            JobRequestData::BlockProof(prover_data, _) => {
                let mut single_proof = self
                    .precomputed_proofs
                    .single_proofs
                    .get(0)
                    .expect("Failed to load correct single proof")
                    .0
                    .clone();
                single_proof.0.input_values[0] = prover_data.public_data_commitment;
                JobResultData::BlockProof(single_proof)
            }
        };
        Ok(empty_proof)
    }
}
