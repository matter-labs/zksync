use zksync_prover::cli_utils::main_for_prover_impl;
use zksync_prover::dummy_prover::DummyProver;

#[tokio::main]
async fn main() {
    main_for_prover_impl::<DummyProver>().await;
}
