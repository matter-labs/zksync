use crate::circuit::FranklinCircuit;
use crate::franklin_crypto::bellman::Circuit;
use crate::franklin_crypto::circuit::test::TestConstraintSystem;
use models::circuit::account::CircuitAccount;
use models::circuit::CircuitAccountTree;
use models::node::{Account, AccountId, Address, Engine};
use plasma::state::PlasmaState;

pub use crate::witness::utils::WitnessBuilder;

pub fn check_circuit(circuit: FranklinCircuit<Engine>) {
    let mut cs = TestConstraintSystem::<Engine>::new();
    circuit.synthesize(&mut cs).unwrap();

    println!("unconstrained: {}", cs.find_unconstrained());
    println!("number of constraints {}", cs.num_constraints());
    if let Some(err) = cs.which_is_unsatisfied() {
        panic!("ERROR satisfying in {}", err);
    }
}

pub fn test_genesis_plasma_state(
    accounts: Vec<(AccountId, Account)>,
) -> (PlasmaState, WitnessBuilder) {
    if accounts.iter().any(|(id, _)| *id == 0) {
        panic!("AccountId 0 is existing fee account");
    }

    let fee_account_id = 0;
    let validator_account = vec![(
        fee_account_id,
        Account::default_with_address(&Address::default()),
    )]
    .into_iter()
    .chain(accounts.into_iter())
    .collect();
    let plasma_state = PlasmaState::new(validator_account, 1);

    let mut circuit_account_tree =
        CircuitAccountTree::new(models::params::account_tree_depth() as u32);
    for (id, account) in plasma_state.get_accounts() {
        circuit_account_tree.insert(id, CircuitAccount::from(account))
    }

    let witness_accum = WitnessBuilder::new(circuit_account_tree, fee_account_id, 1);

    (plasma_state, witness_accum)
}
