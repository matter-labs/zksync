use anyhow::Error;
use zksync_prover::cli_utils::main_for_prover_impl;
use zksync_prover::{ProverConfig, ProverImpl};
use zksync_prover_utils::api::{JobRequestData, JobResultData};
use zksync_prover_utils::fs_utils::{load_correct_aggregated_proof, load_correct_single_proof};
use zksync_utils::parse_env_to_collection;

#[derive(Debug)]
pub struct DummyProverConfig {
    pub block_sizes: Vec<usize>,
}

impl ProverConfig for DummyProverConfig {
    fn from_env() -> Self {
        Self {
            block_sizes: parse_env_to_collection("SUPPORTED_BLOCK_CHUNKS_SIZES"),
        }
    }
}

#[derive(Debug)]
struct DummyProver {
    config: DummyProverConfig,
}

impl ProverImpl for DummyProver {
    type Config = DummyProverConfig;

    fn create_from_config(config: Self::Config) -> Self {
        Self { config }
    }

    fn create_proof(&self, data: JobRequestData) -> Result<JobResultData, Error> {
        let empty_proof = match data {
            JobRequestData::AggregatedBlockProof(single_proofs) => {
                let mut aggregated_proof = load_correct_aggregated_proof()
                    .expect("Failed to load correct aggregated proof");
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
                let mut single_proof =
                    load_correct_single_proof().expect("Failed to load correct single proof");
                single_proof.0.input_values[0] = prover_data.public_data_commitment;
                JobResultData::BlockProof(single_proof)
            }
        };
        Ok(empty_proof)
    }
}

#[tokio::main]
async fn main() {
    main_for_prover_impl::<DummyProver>().await;
}
