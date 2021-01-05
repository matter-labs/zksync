// Built-in deps
use std::{net, str::FromStr, thread, time::Duration};
// External deps
use futures::channel::mpsc;
// Workspace deps
use zksync_config::ProverOptions;
use zksync_prover::{client, ApiClient};
use zksync_prover_utils::api::ProverInputRequest;
// Local deps
use zksync_witness_generator::run_prover_server;

const CORRECT_PROVER_SECRET_AUTH: &str = "42";
const INCORRECT_PROVER_SECRET_AUTH: &str = "123";
const SERVER_BIND_TO: &str = "127.0.0.1:8088";

async fn connect_to_db() -> zksync_storage::ConnectionPool {
    zksync_storage::ConnectionPool::new(Some(1))
}

struct MockProverOptions(ProverOptions);

impl Default for MockProverOptions {
    fn default() -> Self {
        let prover_options = ProverOptions {
            secret_auth: CORRECT_PROVER_SECRET_AUTH.to_string(),
            prepare_data_interval: Duration::from_secs(1),
            heartbeat_interval: Duration::from_millis(1000),
            cycle_wait: Duration::from_millis(500),
            gone_timeout: Duration::from_secs(10),
            prover_server_address: net::SocketAddr::from_str(SERVER_BIND_TO).unwrap(),
            idle_provers: 1,
            witness_generators: 2,
        };
        MockProverOptions(prover_options)
    }
}

async fn spawn_server() {
    let prover_options = MockProverOptions::default();
    let conn_pool = connect_to_db().await;
    let (tx, _rx) = mpsc::channel(1);

    thread::spawn(move || {
        run_prover_server(conn_pool, tx, prover_options.0);
    });
}

#[tokio::test]
async fn test_api_client_with_incorrect_secret_auth() {
    spawn_server().await;
    let client = client::ApiClient::new(
        &format!("http://{}", SERVER_BIND_TO).parse().unwrap(),
        Duration::from_secs(1),
        INCORRECT_PROVER_SECRET_AUTH,
    );

    let get_job_error = &client
        .get_job(ProverInputRequest {
            prover_name: "test".to_string(),
            aux_data: Default::default(),
        })
        .await
        .err()
        .unwrap()
        .to_string();

    assert!(get_job_error.contains("authorization error"));
}
