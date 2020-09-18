use prover::cli_utils::main_for_prover_impl;
use prover::plonk_step_by_step_prover::PlonkStepByStepProver;

fn main() {
    main_for_prover_impl::<PlonkStepByStepProver<prover::client::ApiClient>>();
}
