// Built-in deps
use std::{net::SocketAddr, str::FromStr, thread, time::Duration};
// External deps
use futures::channel::mpsc;
use num::BigUint;
// Workspace deps
use zksync_config::ProverOptions;
use zksync_crypto::franklin_crypto::bellman::pairing::ff::{PrimeField, PrimeFieldRepr};
use zksync_prover::{client, ApiClient};
use zksync_prover_utils::api::ProverInputRequest;
use zksync_types::{block::Block, H256};
// Local deps
use super::mock::MockDatabase;
use crate::{run_prover_server, DatabaseInterface};

const CORRECT_PROVER_SECRET_AUTH: &str = "42";
const INCORRECT_PROVER_SECRET_AUTH: &str = "123";
const SERVER_BIND_TO: &str = "127.0.0.1:8088";

struct MockProverOptions(ProverOptions);

impl Default for MockProverOptions {
    fn default() -> Self {
        let prover_options = ProverOptions {
            secret_auth: CORRECT_PROVER_SECRET_AUTH.to_string(),
            prepare_data_interval: Duration::from_secs(0),
            heartbeat_interval: Duration::from_secs(20),
            cycle_wait: Duration::from_millis(500),
            prover_server_address: SocketAddr::from_str(SERVER_BIND_TO).unwrap(),
            idle_provers: 1,
            witness_generators: 1,
        };
        MockProverOptions(prover_options)
    }
}

async fn spawn_server(database: MockDatabase) {
    let prover_options = MockProverOptions::default();
    let (tx, _rx) = mpsc::channel(1);

    thread::spawn(move || {
        run_prover_server(database, tx, prover_options.0);
    });
}

#[tokio::test]
async fn test_api_client() {
    let database = MockDatabase::new();
    spawn_server(database.clone()).await;
    test_api_client_with_incorrect_secret_auth("tests1").await;
    test_api_client_simple_simulation("test2", database).await;
}

async fn test_api_client_with_incorrect_secret_auth(prover_name: &str) {
    let client = client::ApiClient::new(
        &format!("http://{}", SERVER_BIND_TO).parse().unwrap(),
        Duration::from_secs(1),
        INCORRECT_PROVER_SECRET_AUTH,
    );

    let get_job_error = &client
        .get_job(ProverInputRequest {
            prover_name: prover_name.to_string(),
            aux_data: Default::default(),
        })
        .await
        .err()
        .unwrap()
        .to_string();

    assert!(get_job_error.contains("authorization error"));
}

async fn test_api_client_simple_simulation(prover_name: &str, database: MockDatabase) {
    let client = client::ApiClient::new(
        &format!("http://{}", SERVER_BIND_TO).parse().unwrap(),
        Duration::from_secs(1),
        CORRECT_PROVER_SECRET_AUTH,
    );

    // Call `get_job` and check that data is None.
    let job = client
        .get_job(ProverInputRequest {
            prover_name: prover_name.to_string(),
            aux_data: Default::default(),
        })
        .await
        .unwrap();
    assert!(job.data.is_none());

    let block = get_test_block().await;
    database.add_block(block).await;

    println!("Inserting test block");

    MockDatabase::wait_for_stale_job_stale_idle().await;

    // Should return job.
    let job = client
        .get_job(ProverInputRequest {
            prover_name: prover_name.to_string(),
            aux_data: Default::default(),
        })
        .await
        .unwrap();

    MockDatabase::wait_for_stale_job_stale_idle().await;

    // Should return empty job.
    let next_job = client
        .get_job(ProverInputRequest {
            prover_name: prover_name.to_string(),
            aux_data: Default::default(),
        })
        .await
        .unwrap();
    assert!(job.data.is_some());
    assert!(next_job.data.is_none());

    client
        .prover_stopped(prover_name.to_string())
        .await
        .unwrap();

    MockDatabase::wait_for_stale_job_stale_idle().await;

    // Should return job.
    let job = client
        .get_job(ProverInputRequest {
            prover_name: prover_name.to_string(),
            aux_data: Default::default(),
        })
        .await
        .unwrap();
    assert!(job.data.is_some());

    let mut storage = database.acquire_connection().await.unwrap();
    let witness = database.load_witness(&mut storage, 1).await.unwrap();
    assert!(witness.is_some());
}

pub async fn get_test_block() -> Block {
    let (circuit_tree, accounts) = MockDatabase::get_default_tree_and_accounts();
    let validator_account_id = 0;
    let validator_account = accounts.get(&validator_account_id).unwrap();
    let mut state = zksync_state::state::ZkSyncState::from_acc_map(accounts.clone(), 1);
    let deposit_priority_op = zksync_types::ZkSyncPriorityOp::Deposit(zksync_types::Deposit {
        from: validator_account.address,
        token: 0,
        amount: BigUint::from(10u32),
        to: validator_account.address,
    });
    let mut op_success = state.execute_priority_op(deposit_priority_op.clone());
    let mut ops = Vec::new();
    let mut accounts_updated = Vec::new();

    accounts_updated.append(&mut op_success.updates);

    ops.push(zksync_types::ExecutedOperations::PriorityOp(Box::new(
        zksync_types::ExecutedPriorityOp {
            op: op_success.executed_op,
            priority_op: zksync_types::PriorityOp {
                serial_id: 0,
                data: deposit_priority_op,
                deadline_block: 2,
                eth_hash: H256::zero(),
                eth_block: 10,
            },
            block_index: 1,
            created_at: chrono::Utc::now(),
        },
    )));

    let old_hash = {
        let mut be_bytes = [0u8; 32];
        circuit_tree
            .root_hash()
            .into_repr()
            .write_be(be_bytes.as_mut())
            .expect("Write commit bytes");
        H256::from(be_bytes)
    };

    Block::new_from_available_block_sizes(
        state.block_number,
        state.root_hash(),
        validator_account_id,
        ops,
        (0, 1),
        &[6 as usize],
        1_000_000.into(),
        1_500_000.into(),
        old_hash,
        0 as u64,
    )
}
