use zksync_prover::cli_utils::main_for_prover_impl;
use zksync_prover::plonk_step_by_step_prover::PlonkStepByStepProver;

fn main() {
    main_for_prover_impl::<PlonkStepByStepProver<zksync_prover::client::ApiClient>>();
}
