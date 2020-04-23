// Built-in deps
use std::str::FromStr;
use std::{net, thread, time};
// External deps
use crypto_exports::pairing::ff::{Field, PrimeField};
use futures::channel::mpsc;
// Workspace deps
use circuit::witness::deposit::apply_deposit_tx;
use circuit::witness::deposit::calculate_deposit_operations_from_witness;
use models::circuit::CircuitAccountTree;
use models::node::Address;
use models::params::{account_tree_depth, block_chunk_sizes, total_tokens};
use models::prover_utils::EncodedProofPlonk;
use prover::client;
use prover::ApiClient;
use server::prover_server;
use std::time::Duration;

fn spawn_server(prover_timeout: time::Duration, rounds_interval: time::Duration) -> String {
    // TODO: make single server spawn for all tests
    let bind_to = "127.0.0.1:8088";
    let conn_pool = storage::ConnectionPool::new(Some(1));
    let addr = net::SocketAddr::from_str(bind_to).unwrap();
    let (tx, _rx) = mpsc::channel(1);
    let tree = CircuitAccountTree::new(account_tree_depth());
    thread::spawn(move || {
        prover_server::start_prover_server(
            conn_pool,
            addr,
            prover_timeout,
            rounds_interval,
            tx,
            tree,
            0,
        );
    });
    bind_to.to_string()
}

fn access_storage() -> storage::StorageProcessor {
    storage::ConnectionPool::new(Some(1))
        .access_storage()
        .expect("failed to connect to db")
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

#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn api_client_register_start_and_stop_of_prover() {
    let block_size_chunks = block_chunk_sizes()[0];
    let addr = spawn_server(time::Duration::from_secs(1), time::Duration::from_secs(1));
    let client = client::ApiClient::new(
        &format!("http://{}", &addr).parse().unwrap(),
        "foo",
        Duration::from_secs(1),
    );
    let id = client
        .register_prover(block_size_chunks)
        .expect("failed to register");
    let storage = access_storage();
    storage
        .prover_schema()
        .prover_by_id(id)
        .expect("failed to select registered prover");
    client.prover_stopped(id).expect("unexpected error");
    let prover = storage
        .prover_schema()
        .prover_by_id(id)
        .expect("failed to select registered prover");
    prover.stopped_at.expect("expected not empty");
}

#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn api_client_simple_simulation() {
    let prover_timeout = time::Duration::from_secs(1);
    let rounds_interval = time::Duration::from_secs(10);

    let addr = spawn_server(prover_timeout, rounds_interval);

    let block_size_chunks = block_chunk_sizes()[0];
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

    let storage = access_storage();

    let (op, wanted_prover_data) = test_operation_and_wanted_prover_data(block_size_chunks);

    println!("inserting test operation");
    // write test commit operation to db
    storage
        .chain()
        .block_schema()
        .execute_operation(op)
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
    assert_eq!(prover_data.new_root, Some(wanted_prover_data.new_root));
    assert_eq!(
        prover_data.pub_data_commitment,
        Some(wanted_prover_data.public_data_commitment),
    );
}

pub fn test_operation_and_wanted_prover_data(
    block_size_chunks: usize,
) -> (models::Operation, prover::prover_data::ProverData) {
    let mut circuit_tree =
        models::circuit::CircuitAccountTree::new(models::params::account_tree_depth());
    // insert account and its balance
    let storage = access_storage();

    // Fee account
    let mut accounts = models::node::AccountMap::default();
    let validator_account = models::node::Account::default_with_address(&Address::random());
    let validator_account_id: u32 = 0;
    accounts.insert(validator_account_id, validator_account.clone());

    let mut state = plasma::state::PlasmaState::from_acc_map(accounts, 1);
    println!(
        "acc_number 0, acc {:?}",
        models::circuit::account::CircuitAccount::from(validator_account.clone()).pub_key_hash,
    );
    circuit_tree.insert(
        0,
        models::circuit::account::CircuitAccount::from(validator_account.clone()),
    );
    let initial_root = circuit_tree.root_hash();
    let initial_root2 = circuit_tree.root_hash();
    let deposit_priority_op = models::node::FranklinPriorityOp::Deposit(models::node::Deposit {
        from: validator_account.address,
        token: 0,
        amount: bigdecimal::BigDecimal::from(10),
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
                models::node::AccountUpdate::Create {
                    address: validator_account.address,
                    nonce: validator_account.nonce,
                },
            )],
        )
        .unwrap();
    storage
        .chain()
        .state_schema()
        .apply_state_update(0)
        .unwrap();

    ops.push(models::node::ExecutedOperations::PriorityOp(Box::new(
        models::node::ExecutedPriorityOp {
            op: op_success.executed_op,
            priority_op: models::node::PriorityOp {
                serial_id: 0,
                data: deposit_priority_op.clone(),
                deadline_block: 2,
                eth_fee: bigdecimal::BigDecimal::from(0),
                eth_hash: vec![0; 8],
            },
            block_index: 0,
        },
    )));

    let fee_updates = state.collect_fee(&fees, validator_account_id);
    accounts_updated.extend(fee_updates.into_iter());

    let block = models::node::block::Block {
        block_number: state.block_number,
        new_root_hash: state.root_hash(),
        fee_account: validator_account_id,
        block_transactions: ops,
        processed_priority_ops: (0, 1),
    };

    let mut pub_data = vec![];
    let mut operations = vec![];

    if let models::node::FranklinPriorityOp::Deposit(deposit_op) = deposit_priority_op {
        let deposit_witness = apply_deposit_tx(
            &mut circuit_tree,
            &models::node::operations::DepositOp {
                priority_op: deposit_op,
                account_id: 0,
            },
        );

        let deposit_operations = calculate_deposit_operations_from_witness(&deposit_witness);
        operations.extend(deposit_operations);
        pub_data.extend(deposit_witness.get_pubdata());
    }

    for _ in 0..block_size_chunks - operations.len() {
        operations.push(circuit::witness::noop::noop_operation(
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
            None => models::node::Fr::zero(),
            Some(bal) => bal.value,
        };
        validator_balances.push(Some(balance_value));
    }
    let _: models::node::Fr = circuit_tree.root_hash();
    let (root_after_fee, validator_account_witness) =
        circuit::witness::utils::apply_fee(&mut circuit_tree, block.fee_account, 0, 0);

    assert_eq!(root_after_fee, block.new_root_hash);
    let (validator_audit_path, _) =
        circuit::witness::utils::get_audits(&circuit_tree, block.fee_account as u32, 0);
    let public_data_commitment =
        circuit::witness::utils::public_data_commitment::<models::node::Engine>(
            &pub_data,
            Some(initial_root),
            Some(root_after_fee),
            Some(models::node::Fr::from_str(&block.fee_account.to_string()).unwrap()),
            Some(models::node::Fr::from_str(&(block.block_number).to_string()).unwrap()),
        );

    (
        models::Operation {
            id: None,
            action: models::Action::Commit,
            block: block.clone(),
            accounts_updated,
        },
        prover::prover_data::ProverData {
            public_data_commitment,
            old_root: initial_root2,
            new_root: block.new_root_hash,
            validator_address: models::node::Fr::from_str(&block.fee_account.to_string()).unwrap(),
            operations,
            validator_balances,
            validator_audit_path,
            validator_account: validator_account_witness,
        },
    )
}

#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn api_server_publish_dummy() {
    let prover_timeout = time::Duration::from_secs(1);
    let rounds_interval = time::Duration::from_secs(10);
    let addr = spawn_server(prover_timeout, rounds_interval);

    let client = reqwest::Client::new();
    let res = client
        .post(&format!("http://{}/publish", &addr))
        .json(&client::PublishReq {
            block: 1,
            proof: EncodedProofPlonk::default(),
        })
        .send()
        .expect("failed to send publish request");

    assert_eq!(res.status(), reqwest::StatusCode::OK);
}
