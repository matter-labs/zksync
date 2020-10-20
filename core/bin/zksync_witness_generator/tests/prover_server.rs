// Built-in deps
use std::{net, str::FromStr, thread, time, time::Duration};
// External deps
use futures::channel::mpsc;
use zksync_crypto::pairing::ff::{Field, PrimeField};
// Workspace deps
use num::BigUint;
use zksync_circuit::witness::{deposit::DepositWitness, Witness};
use zksync_config::{ConfigurationOptions, ProverOptions};
use zksync_crypto::{params::total_tokens, proof::EncodedProofPlonk};
use zksync_prover::{client, ApiClient};
use zksync_types::{block::Block, Address};
// Local deps
use zksync_circuit::witness::utils::get_used_subtree_root_hash;
use zksync_witness_generator::run_prover_server;

async fn connect_to_db() -> zksync_storage::ConnectionPool {
    zksync_storage::ConnectionPool::new(Some(1)).await
}

async fn spawn_server(prover_timeout: time::Duration, rounds_interval: time::Duration) -> String {
    // TODO: make single server spawn for all tests
    let bind_to = "127.0.0.1:8088";
    let mut config_opt = ConfigurationOptions::from_env();
    config_opt.prover_server_address = net::SocketAddr::from_str(bind_to).unwrap();

    let mut prover_options = ProverOptions::from_env();
    prover_options.prepare_data_interval = rounds_interval;
    prover_options.gone_timeout = prover_timeout;

    let conn_pool = connect_to_db().await;
    let (tx, _rx) = mpsc::channel(1);

    thread::spawn(move || {
        run_prover_server(conn_pool, tx, prover_options, config_opt);
    });
    bind_to.to_string()
}

#[test]
#[should_panic]
fn client_with_empty_worker_name_panics() {
    client::ApiClient::new(
        &"http:://example.com".parse().unwrap(),
        "",
        Duration::from_secs(1),
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "db_test"), ignore)]
async fn api_client_register_start_and_stop_of_prover() {
    let block_size_chunks = ConfigurationOptions::from_env().available_block_chunk_sizes[0];
    let addr = spawn_server(time::Duration::from_secs(1), time::Duration::from_secs(1)).await;
    let client = client::ApiClient::new(
        &format!("http://{}", &addr).parse().unwrap(),
        "foo",
        Duration::from_secs(1),
    );
    let id = client
        .register_prover(block_size_chunks)
        .expect("failed to register");

    let db_connection = connect_to_db().await;
    let mut storage = db_connection
        .access_storage()
        .await
        .expect("Failed to connect to db");

    storage
        .prover_schema()
        .prover_by_id(id)
        .await
        .expect("failed to select registered prover");
    client.prover_stopped(id).expect("unexpected error");
    let prover = storage
        .prover_schema()
        .prover_by_id(id)
        .await
        .expect("failed to select registered prover");
    prover.stopped_at.expect("expected not empty");
}

#[tokio::test]
#[cfg_attr(not(feature = "db_test"), ignore)]
async fn api_client_simple_simulation() {
    let prover_timeout = time::Duration::from_secs(1);
    let rounds_interval = time::Duration::from_secs(10);

    let addr = spawn_server(prover_timeout, rounds_interval).await;

    let block_size_chunks = ConfigurationOptions::from_env().available_block_chunk_sizes[0];
    let client = client::ApiClient::new(
        &format!("http://{}", &addr).parse().unwrap(),
        "foo",
        time::Duration::from_secs(1),
    );

    // call block_to_prove and check its none
    let to_prove = client
        .block_to_prove(block_size_chunks)
        .expect("failed to get block to prove");
    assert!(to_prove.is_none());

    let db_connection = connect_to_db().await;
    let mut storage = db_connection
        .access_storage()
        .await
        .expect("Failed to connect to db");

    let (op, wanted_prover_data) = test_operation_and_wanted_prover_data(block_size_chunks).await;

    println!("inserting test operation");
    // write test commit operation to db
    storage
        .chain()
        .block_schema()
        .execute_operation(op)
        .await
        .expect("failed to mock commit operation");

    thread::sleep(time::Duration::from_secs(10));

    // should return block
    let to_prove = client
        .block_to_prove(block_size_chunks)
        .expect("failed to bet block to prove");
    assert!(to_prove.is_some());

    // block is taken unless no heartbeat from prover within prover_timeout period
    // should return None at this moment
    let to_prove = client
        .block_to_prove(block_size_chunks)
        .expect("failed to get block to prove");
    assert!(to_prove.is_none());

    // make block available
    thread::sleep(prover_timeout * 10);

    let to_prove = client
        .block_to_prove(block_size_chunks)
        .expect("failed to get block to prove");
    assert!(to_prove.is_some());

    let (block, job) = to_prove.unwrap();
    // sleep for prover_timeout and send heartbeat
    thread::sleep(prover_timeout * 2);
    client.working_on(job).unwrap();

    let to_prove = client
        .block_to_prove(block_size_chunks)
        .expect("failed to get block to prove");
    assert!(to_prove.is_none());

    let prover_data = client
        .prover_data(block)
        .expect("failed to get prover data");
    assert_eq!(prover_data.old_root, Some(wanted_prover_data.old_root));
    assert_eq!(
        prover_data.pub_data_commitment,
        Some(wanted_prover_data.public_data_commitment),
    );
}

pub async fn test_operation_and_wanted_prover_data(
    block_size_chunks: usize,
) -> (
    zksync_types::Operation,
    zksync_prover_utils::prover_data::ProverData,
) {
    let mut circuit_tree = zksync_crypto::circuit::CircuitAccountTree::new(
        zksync_crypto::params::account_tree_depth(),
    );
    // insert account and its balance

    let db_connection = connect_to_db().await;
    let mut storage = db_connection
        .access_storage()
        .await
        .expect("Failed to connect to db");

    // Fee account
    let mut accounts = zksync_types::AccountMap::default();
    let validator_account = zksync_types::Account::default_with_address(&Address::random());
    let validator_account_id: u32 = 0;
    accounts.insert(validator_account_id, validator_account.clone());

    let mut state = zksync_state::state::ZkSyncState::from_acc_map(accounts, 1);
    println!(
        "acc_number 0, acc {:?}",
        zksync_crypto::circuit::account::CircuitAccount::from(validator_account.clone())
            .pub_key_hash,
    );
    circuit_tree.insert(
        0,
        zksync_crypto::circuit::account::CircuitAccount::from(validator_account.clone()),
    );
    let initial_root = circuit_tree.root_hash();
    let initial_root2 = circuit_tree.root_hash();
    let initial_used_subtree_root = get_used_subtree_root_hash(&circuit_tree);
    let deposit_priority_op = zksync_types::ZkSyncPriorityOp::Deposit(zksync_types::Deposit {
        from: validator_account.address,
        token: 0,
        amount: BigUint::from(10u32),
        to: validator_account.address,
    });
    let mut op_success = state.execute_priority_op(deposit_priority_op.clone());
    let mut fees = Vec::new();
    let mut ops = Vec::new();
    let mut accounts_updated = Vec::new();

    if let Some(fee) = op_success.fee {
        fees.push(fee);
    }

    accounts_updated.append(&mut op_success.updates);

    storage
        .chain()
        .state_schema()
        .commit_state_update(
            0,
            &[(
                0,
                zksync_types::AccountUpdate::Create {
                    address: validator_account.address,
                    nonce: validator_account.nonce,
                },
            )],
            0,
        )
        .await
        .unwrap();
    storage
        .chain()
        .state_schema()
        .apply_state_update(0)
        .await
        .unwrap();

    ops.push(zksync_types::ExecutedOperations::PriorityOp(Box::new(
        zksync_types::ExecutedPriorityOp {
            op: op_success.executed_op,
            priority_op: zksync_types::PriorityOp {
                serial_id: 0,
                data: deposit_priority_op.clone(),
                deadline_block: 2,
                eth_hash: vec![0; 8],
                eth_block: 10,
            },
            block_index: 0,
            created_at: chrono::Utc::now(),
        },
    )));

    let fee_updates = state.collect_fee(&fees, validator_account_id);
    accounts_updated.extend(fee_updates.into_iter());

    let block = Block::new_from_available_block_sizes(
        state.block_number,
        state.root_hash(),
        validator_account_id,
        ops,
        (0, 1),
        &ConfigurationOptions::from_env().available_block_chunk_sizes,
        1_000_000.into(),
        1_500_000.into(),
    );

    let mut pub_data = vec![];
    let mut operations = vec![];

    if let zksync_types::ZkSyncPriorityOp::Deposit(deposit_op) = deposit_priority_op {
        let deposit_witness = DepositWitness::apply_tx(
            &mut circuit_tree,
            &zksync_types::operations::DepositOp {
                priority_op: deposit_op,
                account_id: 0,
            },
        );

        let deposit_operations = deposit_witness.calculate_operations(());
        operations.extend(deposit_operations);
        pub_data.extend(deposit_witness.get_pubdata());
    }

    for _ in 0..block_size_chunks - operations.len() {
        operations.push(zksync_circuit::witness::noop::noop_operation(
            &circuit_tree,
            block.fee_account,
        ));
        pub_data.extend(vec![false; 64]);
    }
    assert_eq!(pub_data.len(), 64 * block_size_chunks);
    assert_eq!(operations.len(), block_size_chunks);

    let validator_acc = circuit_tree
        .get(block.fee_account as u32)
        .expect("fee_account is not empty");
    let mut validator_balances = vec![];
    for i in 0..total_tokens() {
        let balance_value = match validator_acc.subtree.get(i as u32) {
            None => zksync_crypto::Fr::zero(),
            Some(bal) => bal.value,
        };
        validator_balances.push(Some(balance_value));
    }
    let _: zksync_crypto::Fr = circuit_tree.root_hash();
    let (root_after_fee, validator_account_witness) =
        zksync_circuit::witness::utils::apply_fee(&mut circuit_tree, block.fee_account, 0, 0);

    assert_eq!(root_after_fee, block.new_root_hash);
    let (validator_audit_path, _) =
        zksync_circuit::witness::utils::get_audits(&circuit_tree, block.fee_account as u32, 0);
    let public_data_commitment =
        zksync_circuit::witness::utils::public_data_commitment::<zksync_crypto::Engine>(
            &pub_data,
            Some(initial_root),
            Some(root_after_fee),
            Some(zksync_crypto::Fr::from_str(&block.fee_account.to_string()).unwrap()),
            Some(zksync_crypto::Fr::from_str(&(block.block_number).to_string()).unwrap()),
        );

    (
        zksync_types::Operation {
            id: None,
            action: zksync_types::Action::Commit,
            block: block.clone(),
        },
        zksync_prover_utils::prover_data::ProverData {
            public_data_commitment,
            old_root: initial_root2,
            initial_used_subtree_root,
            new_root: block.new_root_hash,
            validator_address: zksync_crypto::Fr::from_str(&block.fee_account.to_string()).unwrap(),
            operations,
            validator_balances,
            validator_audit_path,
            validator_account: validator_account_witness,
        },
    )
}

#[tokio::test]
#[cfg_attr(not(feature = "db_test"), ignore)]
async fn api_server_publish_dummy() {
    let prover_timeout = time::Duration::from_secs(1);
    let rounds_interval = time::Duration::from_secs(10);
    let addr = spawn_server(prover_timeout, rounds_interval).await;

    let client = reqwest::Client::new();
    let res = client
        .post(&format!("http://{}/publish", &addr))
        .json(&zksync_prover_utils::api::PublishReq {
            block: 1,
            proof: EncodedProofPlonk::default(),
        })
        .send()
        .await
        .expect("failed to send publish request");

    assert_eq!(res.status(), reqwest::StatusCode::OK);
}
