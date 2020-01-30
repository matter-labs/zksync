use circuit::exit_circuit::create_exit_circuit;
use franklin_crypto::bellman::groth16::create_random_proof;
use models::circuit::account::CircuitAccount;
use models::circuit::CircuitAccountTree;
use rand::thread_rng;
use server::state_keeper::PlasmaStateInitParams;
use storage::ConnectionPool;

fn main() {
    let target_account_id = 0;
    let token_id = 0;

    let connection_pool = ConnectionPool::new();
    let state = PlasmaStateInitParams::restore_from_db(connection_pool.clone());

    let mut circuit_account_tree =
        CircuitAccountTree::new(models::params::account_tree_depth() as u32);
    for (id, account) in state.accounts {
        circuit_account_tree.insert(id, CircuitAccount::from(account));
    }

    let zksync_exit_circuit =
        create_exit_circuit(&mut circuit_account_tree, target_account_id, token_id);

    let p = create_random_proof(instance, &circuit_params, &mut thread_rng())
        .expect("failed to create proof");
    //
    //    let pvk = bellman::groth16::prepare_verifying_key(&circuit_params.vk);
    //
    //    let proof_verified =
    //        bellman::groth16::verify_proof(&pvk, &p.clone(), &[prover_data.public_data_commitment])
    //            .map_err(|e| {
    //                BabyProverError::Internal(format!("failed to verify created proof: {}", e))
    //            })?;
    //    if !proof_verified {
    //        return Err(BabyProverError::Internal(
    //            "created proof did not pass verification".to_owned(),
    //        ));
    //    }
}
