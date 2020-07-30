use crate::{ApiClient, BabyProverError, ProverConfig, ProverHeartbeat, ProverImpl, ProverJob};
use crypto_exports::bellman::pairing::{CurveAffine, Engine as EngineTrait};
use crypto_exports::bellman::plonk::better_better_cs::{setup::VerificationKey, verifier::verify};
use crypto_exports::bellman::plonk::better_cs::{cs::PlonkCsWidth4WithNextStepParams, keys::Proof};
use crypto_exports::bellman::worker::Worker;
use crypto_exports::ff::ScalarEngine;
use crypto_exports::recursive_aggregation_circuit::circuit::{
    create_recursive_circuit_setup, create_zksync_recursive_aggregate,
    proof_recursive_aggregate_for_zksync,
};
use models::config_options::{get_env, parse_env, AvailableBlockSizesConfig};
use models::node::Engine;
use models::params::{
    RECURSIVE_CIRCUIT_NUM_INPUTS, RECURSIVE_CIRCUIT_SIZES, RECURSIVE_CIRCUIT_VK_TREE_DEPTH,
};
use models::primitives::serialize_fe_for_ethereum;
use models::prover_utils::fs_utils::get_recursive_verification_key_path;
use models::prover_utils::{
    get_universal_setup_monomial_form, save_to_cache_universal_setup_monomial_form,
    serialize_new_proof, EncodedProofPlonk,
};
use models::prover_utils::{PlonkVerificationKey, SetupForStepByStepProver};
use std::fs::File;
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
        data: Vec<(EncodedProofPlonk, usize)>,
    ) -> Result<EncodedProofPlonk, BabyProverError> {
        let worker = Worker::new();

        let block_sizes_config = AvailableBlockSizesConfig::from_env();
        let all_vks = block_sizes_config
            .blocks_chunks
            .iter()
            .map(|chunks| {
                PlonkVerificationKey::read_verification_key_for_main_circuit(*chunks)
                    .unwrap()
                    .0
            })
            .collect::<Vec<_>>();
        let mut proofs = Vec::new();
        let mut vk_indexes = Vec::new();
        for (proof, chunks) in data.clone() {
            let proof = Proof::<Engine, PlonkCsWidth4WithNextStepParams>::read(
                proof.proof_binary.as_slice(),
            )
            .expect("Failed to deserialize proof");
            proofs.push(proof);
            let idx = block_sizes_config
                .blocks_chunks
                .iter()
                .position(|block_chunks| *block_chunks == chunks)
                .expect("block size not found in available block sizes");
            vk_indexes.push(idx);
        }

        let setup_power = RECURSIVE_CIRCUIT_SIZES
            .iter()
            .find_map(|(aggr_size, aggregate_setup_power)| {
                if *aggr_size == proofs.len() {
                    Some(*aggregate_setup_power)
                } else {
                    None
                }
            })
            .expect("Aggregate cirucit of correct size not found");

        let universal_setup =
            get_universal_setup_monomial_form(setup_power, self.config.download_setup_from_network)
                .expect("universal_setup");
        let mut g2_bases = [<<Engine as EngineTrait>::G2Affine as CurveAffine>::zero(); 2];
        g2_bases.copy_from_slice(&universal_setup.g2_monomial_bases.as_ref()[..]);
        let aggregate = create_zksync_recursive_aggregate(
            RECURSIVE_CIRCUIT_VK_TREE_DEPTH,
            RECURSIVE_CIRCUIT_NUM_INPUTS,
            &all_vks,
            &proofs,
            &vk_indexes,
            &g2_bases,
        )
        .expect("must create aggregate");
        let aggr_limbs = aggregate
            .limbed_aggregated_g1_elements
            .iter()
            .map(|l| serialize_fe_for_ethereum(l))
            .collect::<Vec<_>>();

        let setup = create_recursive_circuit_setup(
            data.len(),
            RECURSIVE_CIRCUIT_NUM_INPUTS,
            RECURSIVE_CIRCUIT_VK_TREE_DEPTH,
        )
        .expect("failed to create_recursive_circuit_vk_and_setup");

        let vk_for_recursive_circuit = VerificationKey::read(
            File::open(get_recursive_verification_key_path(proofs.len()))
                .expect("recursive verification key not found"),
        )
        .expect("recursive verification key read fail");
        let rec_aggr_proof = proof_recursive_aggregate_for_zksync(
            RECURSIVE_CIRCUIT_VK_TREE_DEPTH,
            RECURSIVE_CIRCUIT_NUM_INPUTS,
            &all_vks,
            &proofs,
            &vk_indexes,
            &vk_for_recursive_circuit,
            &setup,
            &universal_setup,
            true,
            &worker,
        )
        .expect("must create aggregate");
        save_to_cache_universal_setup_monomial_form(setup_power, universal_setup);

        use crypto_exports::franklin_crypto::bellman::plonk::commitments::transcript::keccak_transcript::RollingKeccakTranscript;
        let is_valid = verify::<_, _, RollingKeccakTranscript<<Engine as ScalarEngine>::Fr>>(
            &vk_for_recursive_circuit,
            &rec_aggr_proof,
            None,
        )
        .expect("must perform verification");
        if !is_valid {
            return Err(BabyProverError::Internal("Proof is invalid".to_string()));
        }
        let (inputs, proof) = serialize_new_proof(&rec_aggr_proof);
        let res = EncodedProofPlonk {
            inputs,
            proof,
            proof_binary: Vec::new(),
            subproof_limbs: aggr_limbs,
        };
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
