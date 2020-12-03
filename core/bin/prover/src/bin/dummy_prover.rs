use anyhow::Error;
use std::sync::mpsc;
use std::time::Duration;
use zksync_prover::cli_utils::main_for_prover_impl;
use zksync_prover::{ApiClient, ProverConfig, ProverImpl};
use zksync_prover_utils::api::{JobRequestData, JobResultData};
use zksync_utils::get_env;

#[derive(Debug)]
pub struct DummyProverConfig {
    pub block_sizes: Vec<usize>,
}

impl ProverConfig for DummyProverConfig {
    fn from_env() -> Self {
        Self {
            block_sizes: get_env("SUPPORTED_BLOCK_CHUNKS_SIZES")
                .split(',')
                .map(|p| p.parse().unwrap())
                .collect(),
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
            JobRequestData::AggregatedBlockProof(_) => {
                JobResultData::AggregatedBlockProof(Default::default())
            }
            JobRequestData::BlockProof(..) => JobResultData::BlockProof(Default::default()),
        };
        Ok(empty_proof)
    }
}

#[tokio::main]
async fn main() {
    main_for_prover_impl::<DummyProver>().await;
}
