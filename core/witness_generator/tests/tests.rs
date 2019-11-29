// Built-in uses
use std::str::FromStr;
use std::{net, thread, time};
// External uses
use ff::{Field};
// Workspace uses
use witness_generator::{client, start_server};
use prover::ApiClient;


fn spawn_server(prover_timeout: time::Duration) -> String {
    // TODO: make single server spawn for all tests
    let bind_to = "127.0.0.1:8088";
    let addr = net::SocketAddr::from_str(bind_to).unwrap();
    thread::spawn(move || {
        start_server(&addr, prover_timeout);
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
fn client_with_empty_worker_name() {
    client::ApiClient::new("", "");
}

#[test]
fn register_prover() {
    let addr = spawn_server(time::Duration::from_secs(1));
    let client = client::ApiClient::new(&format!("http://{}", &addr), "foo");
    let id = client.register_prover().expect("failed to register");
    let storage = access_storage();
    storage.prover_by_id(id).expect("failed to select registered prover");
}

#[test]
fn api_client_block_to_prove_and_working_on(){
    let prover_timeout = time::Duration::from_secs(1);
    let addr = spawn_server(prover_timeout);
    let client = client::ApiClient::new(&format!("http://{}", &addr), "foo");
    // call block_to_prove and check its none
    let block = client.block_to_prove().expect("failed to get block to prove");
    assert_eq!(block, None);

    // use storage to mock some data
    let storage = access_storage();
    let op = models::Operation {
        action: models::Action::Commit,
        accounts_updated: Vec::new(),
        block: models::node::block::Block{
            block_number: 1,
            new_root_hash: models::node::Fr::zero(),
            fee_account: 0,
            block_transactions: Vec::new(),
            processed_priority_ops: (0, 0),
        },
        id: None,
    };
    storage.execute_operation(&op).expect("failed to mock commit operation");
    // should return block to prove
    let block = client.block_to_prove().expect("failed to bet block to prove");
    assert_eq!(Some(1), block);
    // block is taken by now, should return None
    let block = client.block_to_prove().expect("failed to get block to prove");
    assert!(block.is_none());
    // make block available
    thread::sleep(prover_timeout * 2);
    let block = client.block_to_prove().expect("failed to get block to prove");
    assert_eq!(Some(1), block);

    // sleep for prover_timeout but let others know block is being verified
    thread::sleep(prover_timeout * 2);
    client.working_on(block.unwrap());
    let block = client.block_to_prove().expect("failed to get block to prove");
    assert!(block.is_none());
}
