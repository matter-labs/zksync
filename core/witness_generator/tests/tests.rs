// Built-in uses
use std::str::FromStr;
use std::{net, thread, time};
// External uses
use ff::{Field, PrimeField};
use rand::Rng;
// Workspace uses
use witness_generator::{client, server};
use prover::ApiClient;


fn spawn_server(prover_timeout: time::Duration, rounds_interval: time::Duration) -> String {
    // TODO: make single server spawn for all tests
    let bind_to = "127.0.0.1:8088";
    let addr = net::SocketAddr::from_str(bind_to).unwrap();
    thread::spawn(move || {
        server::start_server(&addr, prover_timeout, rounds_interval);
    });
    bind_to.to_string()
}

fn access_storage() -> storage::StorageProcessor {
    storage::ConnectionPool::new()
        .access_storage()
        .expect("failed to connect to db")
}

#[test]
#[should_panic]
fn client_with_empty_worker_name_panics() {
    client::ApiClient::new("", "");
}

#[test]
fn api_client_register_prover() {
    let addr = spawn_server(time::Duration::from_secs(1), time::Duration::from_secs(1));
    let client = client::ApiClient::new(&format!("http://{}", &addr), "foo");
    let id = client.register_prover().expect("failed to register");
    let storage = access_storage();
    storage.prover_by_id(id).expect("failed to select registered prover");
}

/// TestAccount is an account with random generated keys and address.
struct TestAccount {
    pub private_key: franklin_crypto::eddsa::PrivateKey<pairing::bn256::Bn256>,
    pub public_key: franklin_crypto::eddsa::PublicKey<pairing::bn256::Bn256>,
    pub address: models::node::account::AccountAddress
}

// TODO: move to helper crate
impl TestAccount {
    pub fn new() -> Self {
        let rng = &mut rand::thread_rng();
        let p_g = franklin_crypto::alt_babyjubjub::FixedGenerators::SpendingKeyGenerator;
        let jubjub_params = &franklin_crypto::alt_babyjubjub::AltJubjubBn256::new();
        let private_key = franklin_crypto::eddsa::PrivateKey::<pairing::bn256::Bn256>(rng.gen());
        let public_key = franklin_crypto::eddsa::PublicKey::<pairing::bn256::Bn256>::from_private(
            &private_key,
            p_g,
            jubjub_params,
        );
        let address = models::node::account::AccountAddress::from_pubkey(public_key);
        let public_key = franklin_crypto::eddsa::PublicKey::<pairing::bn256::Bn256>::from_private(
            &private_key,
            p_g,
            jubjub_params,
        );
        TestAccount{
            private_key,
            public_key,
            address,
        }
    }
}

fn test_operation_and_wanted_prover_data() -> (models::Operation, prover::ProverData) {
    let mut circuit_tree = models::circuit::CircuitAccountTree::new(models::params::account_tree_depth() as u32);

    let validator_test_account = TestAccount::new();

    // Fee account
    let mut accounts = models::node::AccountMap::default();
    let mut validator_account = models::node::Account::default();
    validator_account.address = validator_test_account.address.clone();
    let validator_account_id: u32 = 0;
    accounts.insert(validator_account_id, validator_account.clone());

    let mut state = plasma::state::PlasmaState::new(accounts, 1);
    let genesis_root_hash = state.root_hash();
    circuit_tree.insert(0, models::circuit::account::CircuitAccount::from(validator_account.clone()));
    assert_eq!(circuit_tree.root_hash(), genesis_root_hash);

    let deposit_priority_op = models::node::FranklinPriorityOp::Deposit(
        models::node::Deposit{
            sender: web3::types::Address::zero(),
            token: 0,
            amount: bigdecimal::BigDecimal::from(10),
            account: validator_test_account.address.clone(),
        },
    );
    let mut op_success = state.execute_priority_op(deposit_priority_op.clone());
    let mut fees = Vec::new();
    let mut ops = Vec::new();
    let mut accounts_updated = Vec::new();

    if let Some(fee) = op_success.fee {
        fees.push(fee);
    }

    accounts_updated.append(&mut op_success.updates);

    ops.push(models::node::ExecutedOperations::PriorityOp(Box::new(models::node::ExecutedPriorityOp{
        op: op_success.executed_op,
        priority_op: models::node::PriorityOp{
            serial_id: 0,
            data: deposit_priority_op.clone(),
            deadline_block: 2,
            eth_fee: bigdecimal::BigDecimal::from(0),
            eth_hash: vec![0; 8],
        },
        block_index: 0,
    })));

    let (fee_account_id, fee_updates) = state.collect_fee(&fees, &validator_test_account.address);
    accounts_updated.extend(fee_updates.into_iter());

    let block = models::node::block::Block {
        block_number: state.block_number,
        new_root_hash: state.root_hash(),
        fee_account: fee_account_id,
        block_transactions: ops,
        processed_priority_ops: (0, 1),
    };

    let mut pub_data = vec![];
    let mut operations = vec![];

    if let models::node::FranklinPriorityOp::Deposit(deposit_op) = deposit_priority_op {
        let deposit_witness = circuit::witness::deposit::apply_deposit_tx(&mut circuit_tree, &models::node::operations::DepositOp{
            priority_op: deposit_op,
            account_id: 0,
        });

        let deposit_operations = circuit::witness::deposit::calculate_deposit_operations_from_witness(
            &deposit_witness,
            &models::node::Fr::zero(),
            &models::node::Fr::zero(),
            &models::node::Fr::zero(),
            &circuit::operation::SignatureData{
                r_packed: vec![Some(false); 256],
                s: vec![Some(false); 256],
            },
            &[Some(false); 256],
        );
        operations.extend(deposit_operations);
        pub_data.extend(deposit_witness.get_pubdata());
    }

    let phaser = models::merkle_tree::PedersenHasher::<models::node::Engine>::default();
    let jubjub_params = &franklin_crypto::alt_babyjubjub::AltJubjubBn256::new();
    for _ in 0..models::params::block_size_chunks() - operations.len() {
        let (
            signature,
            first_sig_msg,
            second_sig_msg,
            third_sig_msg,
            _a,
            _b,
        ) = circuit::witness::utils::generate_dummy_sig_data(&[false], &phaser, &jubjub_params);

        operations.push(circuit::witness::noop::noop_operation(
            &circuit_tree,
            block.fee_account,
            &first_sig_msg,
            &second_sig_msg,
            &third_sig_msg,
            &signature,
            &[Some(false); 256],
        ));
        pub_data.extend(vec![false; 64]);
    }
    assert_eq!(pub_data.len(), 64 * models::params::block_size_chunks());
    assert_eq!(operations.len(), models::params::block_size_chunks());

    let validator_acc = circuit_tree
        .get(block.fee_account as u32)
        .expect("fee_account is not empty");
    let mut validator_balances = vec![];
    for i in 0..1 << models::params::BALANCE_TREE_DEPTH {
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

    let public_data_commitment = circuit::witness::utils::public_data_commitment::<models::node::Engine>(
        &pub_data,
        Some(genesis_root_hash),
        Some(root_after_fee),
        Some(models::node::Fr::from_str(&block.fee_account.to_string()).unwrap()),
        Some(models::node::Fr::from_str(&(block.block_number).to_string()).unwrap()),
    );

    (models::Operation{
        id: None,
        action: models::Action::Commit,
        block: block.clone(),
        accounts_updated,
    }, prover::ProverData{
        public_data_commitment,
        old_root: genesis_root_hash,
        new_root: block.new_root_hash,
        validator_address: models::node::Fr::from_str(&block.fee_account.to_string()).unwrap(),
        operations,
        validator_balances,
        validator_audit_path,
        validator_account: validator_account_witness,
    })
}

#[test]
fn api_client_simple_simulation(){
    let prover_timeout = time::Duration::from_secs(1);
    let rounds_interval = time::Duration::from_millis(100);

    let addr = spawn_server(prover_timeout, rounds_interval);

    let client = client::ApiClient::new(&format!("http://{}", &addr), "foo");

    // call block_to_prove and check its none
    let block = client.block_to_prove().expect("failed to get block to prove");
    assert_eq!(block, None);

    let storage = access_storage();

    let (op, _wanted_prover_data) = test_operation_and_wanted_prover_data();

    // write test commit operation to db
    storage.execute_operation(&op).expect("failed to mock commit operation");

    // should return block
    let block = client.block_to_prove().expect("failed to bet block to prove");
    assert_eq!(Some(1), block);

    // block is taken unless no heartbeat from prover within prover_timeout period
    // should return None at this moment
    let block = client.block_to_prove().expect("failed to get block to prove");
    assert!(block.is_none());

    // make block available
    thread::sleep(prover_timeout * 2);

    let block = client.block_to_prove().expect("failed to get block to prove");
    assert_eq!(Some(1), block);

    // sleep for prover_timeout and send heartbeat
    thread::sleep(prover_timeout * 2);
    client.working_on(block.unwrap());

    let block = client.block_to_prove().expect("failed to get block to prove");
    assert!(block.is_none());

    // let prover_data = client.prover_data(1).expect("failed to get prover data");
    // assert_eq!(prover_data.public_data_commitment, wanted_prover_data.public_data_commitment);
}
