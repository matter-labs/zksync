use std::sync::mpsc;
use std::time::Duration;
use zksync_crypto::proof::EncodedProofPlonk;
use zksync_prover::cli_utils::main_for_prover_impl;
use zksync_prover::{ApiClient, BabyProverError, ProverConfig, ProverImpl};
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
struct DummyProver<C> {
    api_client: C,
    heartbeat_interval: Duration,
    config: DummyProverConfig,
}

impl<C: ApiClient> ProverImpl<C> for DummyProver<C> {
    type Config = DummyProverConfig;

    fn create_from_config(
        config: DummyProverConfig,
        api_client: C,
        heartbeat_interval: Duration,
    ) -> Self {
        DummyProver {
            api_client,
            heartbeat_interval,
            config,
        }
    }

    fn next_round(
        &self,
        start_heartbeats_tx: mpsc::Sender<(i32, bool)>,
    ) -> Result<(), BabyProverError> {
        let mut block = 0;
        let mut job_id = 0;

        for block_size in &self.config.block_sizes {
            let block_to_prove = self.api_client.block_to_prove(*block_size).map_err(|e| {
                let e = format!("failed to get block to prove {}", e);
                BabyProverError::Api(e)
            })?;

            let (current_request_block, current_request_job_id) =
                block_to_prove.unwrap_or_else(|| {
                    log::trace!("no block to prove from the server for size: {}", block_size);
                    (0, 0)
                });

            if current_request_job_id != 0 {
                block = current_request_block;
                job_id = current_request_job_id;
                break;
            }
        }

        // Notify heartbeat routine on new proving block job or None.
        start_heartbeats_tx
            .send((job_id, false))
            .expect("failed to send new job to heartbeat routine");
        if job_id == 0 {
            return Ok(());
        }

        log::info!("got job id: {}, block {}", job_id, block);
        let _instance = self.api_client.prover_data(block).map_err(|err| {
            BabyProverError::Api(format!(
                "could not get prover data for block {}: {}",
                block, err
            ))
        })?;

        log::info!("starting to compute proof for block {}", block,);

        self.api_client
            .publish(block, EncodedProofPlonk::default())
            .map_err(|e| BabyProverError::Api(format!("failed to publish proof: {}", e)))?;

        log::info!("finished and published proof for block {}", block);
        Ok(())
    }

    fn get_heartbeat_options(&self) -> (&C, Duration) {
        (&self.api_client, self.heartbeat_interval)
    }
}

fn main() {
    main_for_prover_impl::<DummyProver<zksync_prover::client::ApiClient>>();
}
