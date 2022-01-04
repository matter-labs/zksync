use zksync_prover::cli_utils::main_for_prover_impl;
use zksync_prover::dummy_prover::DummyProver;

#[tokio::main]
async fn main() {
    let run_prometheus_exporter = false;
    main_for_prover_impl::<DummyProver>(run_prometheus_exporter).await;
}
