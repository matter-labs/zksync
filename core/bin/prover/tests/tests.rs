// Built-in deps
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
// External deps
use futures::{pin_mut, FutureExt};
use num::BigUint;
use tokio::sync::Mutex;
// Workspace deps
use zksync_circuit::{
    serialization::ProverData,
    witness::{deposit::DepositWitness, utils::WitnessBuilder, Witness},
};
use zksync_config::{ConfigurationOptions, ProverOptions};
use zksync_crypto::{
    circuit::{account::CircuitAccount, CircuitAccountTree},
    pairing::ff::PrimeField,
    Fr,
};
use zksync_prover::dummy_prover::{DummyProver, DummyProverConfig};
use zksync_prover::plonk_step_by_step_prover::{
    PlonkStepByStepProver, PlonkStepByStepProverConfig,
};
use zksync_prover::{ProverImpl, ShutdownRequest};
use zksync_prover_utils::api::{
    JobRequestData, ProverInputRequest, ProverInputResponse, ProverOutputRequest,
};
use zksync_types::{
    block::smallest_block_size_for_chunks, operations::DepositOp, Account, Address, Deposit,
};

/// Set of different parameters needed for the prover to work
/// Usually, these variables are taken from the environment, but the tests use standard hardcoded values.
struct MockProverConfigs {
    plonk_config: PlonkStepByStepProverConfig,
    dummy_config: DummyProverConfig,
    prover_options: ProverOptions,
    shutdown_request: ShutdownRequest,
    prover_name: String,
}

impl Default for MockProverConfigs {
    fn default() -> Self {
        let plonk_config = PlonkStepByStepProverConfig {
            all_block_sizes: vec![6, 30, 74, 150, 320, 630],
            aggregated_proof_sizes_with_setup_pow: vec![(1, 22), (5, 24), (10, 25), (20, 26)],
            block_sizes: vec![6, 30],
            download_setup_from_network: false,
        };
        let dummy_config = DummyProverConfig {
            block_sizes: vec![6, 30],
        };
        let prover_options = ProverOptions {
            secret_auth: "".to_string(),
            prepare_data_interval: Duration::from_millis(5000),
            heartbeat_interval: Duration::from_millis(0),
            cycle_wait: Duration::from_millis(500),
            gone_timeout: Duration::from_millis(60000),
            prover_server_address: "0.0.0.0:8088"
                .parse::<SocketAddr>()
                .expect("Can't get address from port"),
            witness_generators: 2,
            idle_provers: 1,
        };

        Self {
            plonk_config,
            dummy_config,
            prover_options,
            shutdown_request: Default::default(),
            prover_name: "Test".to_string(),
        }
    }
}

fn test_data_for_prover() -> JobRequestData {
    let mut circuit_account_tree =
        CircuitAccountTree::new(zksync_crypto::params::account_tree_depth());
    let fee_account_id = 0;

    // Init the fee account.
    let fee_account = Account::default_with_address(&Address::default());
    circuit_account_tree.insert(fee_account_id, CircuitAccount::from(fee_account));

    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, fee_account_id, 1, 0);

    let empty_account_id = 1;
    let empty_account_address = [7u8; 20].into();
    let deposit_op = DepositOp {
        priority_op: Deposit {
            from: empty_account_address,
            token: 0,
            amount: BigUint::from(1u32),
            to: empty_account_address,
        },
        account_id: empty_account_id,
    };

    let deposit_witness = DepositWitness::apply_tx(&mut witness_accum.account_tree, &deposit_op);
    let deposit_operations = deposit_witness.calculate_operations(());
    let pub_data_from_witness = deposit_witness.get_pubdata();
    let offset_commitment = deposit_witness.get_offset_commitment_data();

    witness_accum.add_operation_with_pubdata(
        deposit_operations,
        pub_data_from_witness,
        offset_commitment,
    );
    witness_accum.extend_pubdata_with_noops(smallest_block_size_for_chunks(
        DepositOp::CHUNKS,
        &ConfigurationOptions::from_env().available_block_chunk_sizes,
    ));
    witness_accum.collect_fees(&Vec::new());
    witness_accum.calculate_pubdata_commitment();

    let prover_data = ProverData {
        public_data_commitment: witness_accum.pubdata_commitment.unwrap(),
        old_root: witness_accum.initial_root_hash,
        initial_used_subtree_root: witness_accum.initial_used_subtree_root_hash,
        new_root: witness_accum.root_after_fees.unwrap(),
        operations: witness_accum.operations,
        validator_balances: witness_accum.fee_account_balances.unwrap(),
        validator_audit_path: witness_accum.fee_account_audit_path.unwrap(),
        validator_account: witness_accum.fee_account_witness.unwrap(),
        validator_address: Fr::from_str(&witness_accum.fee_account_id.to_string())
            .expect("failed to parse"),
        block_timestamp: Fr::from_str(&witness_accum.timestamp.to_string())
            .expect("failed to parse"),
        block_number: Fr::from_str(&witness_accum.block_number.to_string())
            .expect("failed to parse"),
    };

    JobRequestData::BlockProof(prover_data, 6)
}

#[tokio::test]
async fn test_shutdown_request() {
    let MockProverConfigs {
        plonk_config,
        dummy_config: _,
        prover_options,
        shutdown_request,
        prover_name,
    } = MockProverConfigs::default();

    let prover = PlonkStepByStepProver::create_from_config(plonk_config);
    let client = MockApiClient::default();

    let prover_work_cycle = zksync_prover::prover_work_cycle(
        prover,
        client,
        shutdown_request.clone(),
        prover_options.clone(),
        &prover_name,
    )
    .fuse();
    let timeout = tokio::time::delay_for(prover_options.cycle_wait).fuse();

    pin_mut!(prover_work_cycle, timeout);

    shutdown_request.set();

    let shutdown_requested = futures::select! {
        _ = prover_work_cycle => true,
        _ = timeout => false,
    };

    assert!(
        shutdown_requested,
        "prover did not complete work after receiving a shutdown request"
    );
}

#[tokio::test]
async fn test_receiving_heartbeats() {
    let MockProverConfigs {
        plonk_config,
        dummy_config: _,
        prover_options,
        shutdown_request,
        prover_name,
    } = MockProverConfigs::default();

    let prover = PlonkStepByStepProver::create_from_config(plonk_config);
    let client = MockApiClient::default();

    let prover_work_cycle = zksync_prover::prover_work_cycle(
        prover,
        client.clone(),
        shutdown_request.clone(),
        prover_options.clone(),
        &prover_name,
    )
    .fuse();
    let timeout = tokio::time::delay_for(Duration::from_secs(10)).fuse();

    pin_mut!(prover_work_cycle, timeout);

    futures::select! {
        _ = prover_work_cycle => panic!("prover work ended too quickly"),
        _ = timeout => {
            shutdown_request.set();
            assert_eq!(
                client.working_on.lock().await.get(&0).cloned(),
                Some("Test".to_string())
            );
        },
    };
}

#[tokio::test]
async fn test_publishing_proof() {
    let MockProverConfigs {
        plonk_config: _,
        dummy_config,
        prover_options,
        shutdown_request,
        prover_name,
    } = MockProverConfigs::default();

    let prover = DummyProver::create_from_config(dummy_config);
    let client = MockApiClient::default();

    let prover_work_cycle = zksync_prover::prover_work_cycle(
        prover,
        client.clone(),
        shutdown_request.clone(),
        prover_options.clone(),
        &prover_name,
    )
    .fuse();
    let timeout = tokio::time::delay_for(Duration::from_secs(10)).fuse();

    pin_mut!(prover_work_cycle, timeout);

    futures::select! {
        _ = prover_work_cycle => panic!("prover work ended too quickly"),
        _ = timeout => {
            shutdown_request.set();
            assert!(
                client.published_prof.lock().await.get(&0).cloned().is_some()
            );
        },
    };
}

#[derive(Debug, Clone, Default)]
struct MockApiClient {
    /// All published proofs are saved by `job_id`.
    published_prof: Arc<Mutex<HashMap<i32, ProverOutputRequest>>>,
    /// Received heartbeats from `self.working_on()`.
    working_on: Arc<Mutex<HashMap<i32, String>>>,
    /// `gob_id` of the last work that has not yet been submitted.
    last_job_id: Arc<Mutex<i32>>,
}

#[async_trait::async_trait]
impl zksync_prover::ApiClient for MockApiClient {
    async fn get_job(&self, _: ProverInputRequest) -> anyhow::Result<ProverInputResponse> {
        let last_job_id = *self.last_job_id.lock().await;
        *self.last_job_id.lock().await += 1;
        let response = ProverInputResponse {
            job_id: last_job_id,
            first_block: 1,
            last_block: 1,
            data: Some(test_data_for_prover()),
        };

        Ok(response)
    }

    async fn working_on(&self, job_id: i32, prover_name: &str) -> anyhow::Result<()> {
        self.working_on
            .lock()
            .await
            .insert(job_id, prover_name.to_string());

        Ok(())
    }

    async fn publish(&self, data: ProverOutputRequest) -> anyhow::Result<()> {
        self.published_prof.lock().await.insert(data.job_id, data);

        Ok(())
    }

    async fn prover_stopped(&self, _: String) -> anyhow::Result<()> {
        Ok(())
    }
}
