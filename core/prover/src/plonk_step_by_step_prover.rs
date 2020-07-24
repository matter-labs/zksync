use crate::{ApiClient, BabyProverError, ProverConfig, ProverHeartbeat, ProverImpl, ProverJob};
use models::config_options::{get_env, parse_env};
use models::prover_utils::EncodedProofPlonk;
use models::prover_utils::{PlonkVerificationKey, SetupForStepByStepProver};
use std::sync::{mpsc, Mutex};
use std::time::Duration;

/// We prepare some data before making proof for each block size, so we cache it in case next block
/// would be of our size
struct PreparedComputations {
    block_size: usize,
    setup: SetupForStepByStepProver,
}

pub struct PlonkStepByStepProver<C: ApiClient> {
    config: PlonkStepByStepProverConfig,
    prepared_computations: Mutex<Option<PreparedComputations>>,
    api_client: C,
    heartbeat_interval: Duration,
}

impl<C: ApiClient> PlonkStepByStepProver<C> {
    fn generate_multiblock_proof(
        &self,
        _data: Vec<(EncodedProofPlonk, usize)>,
    ) -> Result<EncodedProofPlonk, BabyProverError> {
        // TODO (AV)
        let mut res = EncodedProofPlonk::default();
        use models::node::U256;
        res.proof[0] = U256::from(15);
        Ok(res)
    }
}

pub struct PlonkStepByStepProverConfig {
    pub block_sizes: Vec<usize>,
    pub download_setup_from_network: bool,
}

impl ProverConfig for PlonkStepByStepProverConfig {
    fn from_env() -> Self {
        Self {
            block_sizes: get_env("BLOCK_CHUNK_SIZES")
                .split(',')
                .map(|p| p.parse().unwrap())
                .collect(),
            download_setup_from_network: parse_env("PROVER_DOWNLOAD_SETUP"),
        }
    }
}

impl<C: ApiClient> ProverImpl<C> for PlonkStepByStepProver<C> {
    type Config = PlonkStepByStepProverConfig;

    fn create_from_config(
        config: PlonkStepByStepProverConfig,
        api_client: C,
        heartbeat_interval: Duration,
    ) -> Self {
        assert!(!config.block_sizes.is_empty());
        PlonkStepByStepProver {
            config,
            prepared_computations: Mutex::new(None),
            api_client,
            heartbeat_interval,
        }
    }

    fn next_round(
        &self,
        start_heartbeats_tx: mpsc::Sender<ProverHeartbeat>,
    ) -> Result<(), BabyProverError> {
        // At start we should try to prove multiblock circuit
        if let Some(((block_from, block_to), job_id)) =
            self.api_client.multiblock_to_prove().map_err(|e| {
                let e = format!("failed to get multiblock to prove {}", e);
                BabyProverError::Api(e)
            })?
        {
            // Notify heartbeat routine on new proving block job or None.
            start_heartbeats_tx
                .send(ProverHeartbeat::WorkingOn(ProverJob::MultiblockProve(
                    job_id,
                )))
                .expect("failed to send new job to heartbeat routine");
            let multiblock_prover_data = self
                .api_client
                .prover_multiblock_data(block_from, block_to)
                .map_err(|err| {
                    BabyProverError::Api(format!(
                        "could not get prover multiblock data for blocks [{};{}]: {}",
                        block_from, block_to, err
                    ))
                })?;

            log::info!(
                "starting to compute multiblock proof for blocks [{};{}]",
                block_from,
                block_to
            );

            let verified_multiblock_proof = self
                .generate_multiblock_proof(multiblock_prover_data)
                .map_err(|e| {
                    BabyProverError::Internal(format!(
                        "Failed to create multiblock verified proof for blocks: [{};{}], err: {}",
                        block_from, block_to, e
                    ))
                })?;

            self.api_client
                .publish_multiblock(block_from, block_to, verified_multiblock_proof)
                .map_err(|e| {
                    BabyProverError::Api(format!("failed to publish multiblock proof: {}", e))
                })?;

            log::info!(
                "finished and published multiblock proof for blocks [{};{}]",
                block_from,
                block_to
            );
            Ok(())
        } else {
            // first we try last proved block, since we have precomputations for it
            let block_size_idx_to_try_first =
                if let Some(precomp) = self.prepared_computations.lock().unwrap().as_ref() {
                    self.config
                        .block_sizes
                        .iter()
                        .position(|size| *size == precomp.block_size)
                        .unwrap()
                } else {
                    0
                };

            let (mut block, mut job_id, mut block_size) = (0, 0, 0);
            for offset_idx in 0..self.config.block_sizes.len() {
                let idx =
                    (block_size_idx_to_try_first + offset_idx) % self.config.block_sizes.len();
                let current_block_size = self.config.block_sizes[idx];

                let block_to_prove =
                    self.api_client
                        .block_to_prove(current_block_size)
                        .map_err(|e| {
                            let e = format!("failed to get block to prove {}", e);
                            BabyProverError::Api(e)
                        })?;

                let (current_request_block, current_request_job_id) = block_to_prove
                    .unwrap_or_else(|| {
                        log::trace!(
                            "no block to prove from the server for size: {}",
                            current_block_size
                        );
                        (0, 0)
                    });

                if current_request_job_id != 0 {
                    block = current_request_block;
                    job_id = current_request_job_id;
                    block_size = current_block_size;
                    break;
                }
            }

            // Notify heartbeat routine on new proving block job or None.
            start_heartbeats_tx
                .send(ProverHeartbeat::WorkingOn(ProverJob::BlockProve(job_id)))
                .expect("failed to send new job to heartbeat routine");
            if job_id == 0 {
                return Ok(());
            }
            let instance = self.api_client.prover_block_data(block).map_err(|err| {
                BabyProverError::Api(format!(
                    "could not get prover data for block {}: {}",
                    block, err
                ))
            })?;

            log::info!(
                "starting to compute proof for block {}, size: {}",
                block,
                block_size
            );

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
                    instance.clone(),
                    self.config.download_setup_from_network,
                )
                .map_err(|e| {
                    BabyProverError::Internal(format!(
                        "Failed to prepare setup for block_size: {}, err: {}",
                        block_size, e
                    ))
                })?;
                PreparedComputations { block_size, setup }
            };

            let vk = PlonkVerificationKey::read_verification_key_for_main_circuit(block_size)
                .map_err(|e| {
                    BabyProverError::Internal(format!(
                        "Failed to read vk for block: {}, size: {}, err: {}",
                        block, block_size, e
                    ))
                })?;
            let verified_proof = precomp
                .setup
                .gen_step_by_step_proof_using_prepared_setup(instance, &vk)
                .map_err(|e| {
                    BabyProverError::Internal(format!(
                        "Failed to create verified proof for block: {}, size: {}, err: {}",
                        block, block_size, e
                    ))
                })?;

            *self.prepared_computations.lock().unwrap() = Some(precomp);

            self.api_client
                .publish_block(block, verified_proof)
                .map_err(|e| {
                    BabyProverError::Api(format!("failed to publish block proof: {}", e))
                })?;

            log::info!("finished and published proof for block {}", block);
            Ok(())
        }
    }

    fn get_heartbeat_options(&self) -> (&C, Duration) {
        (&self.api_client, self.heartbeat_interval)
    }
}
