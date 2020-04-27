// Workspace deps
use models::{
    circuit::{account::CircuitAccount, CircuitAccountTree},
    node::{Account, AccountId, Address, Engine},
};
use plasma::state::PlasmaState;
// Local deps
use crate::{
    circuit::FranklinCircuit,
    franklin_crypto::{bellman::Circuit, circuit::test::TestConstraintSystem},
};

// Public re-exports
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
) -> (PlasmaState, CircuitAccountTree) {
    const FEE_ACCOUNT_ID: u32 = 0;
    if accounts.iter().any(|(id, _)| *id == FEE_ACCOUNT_ID) {
        panic!("AccountId {} is existing fee account", FEE_ACCOUNT_ID);
    }

    let validator_account = std::iter::once((
        FEE_ACCOUNT_ID,
        Account::default_with_address(&Address::default()),
    ))
    .chain(accounts)
    .collect();
    let plasma_state = PlasmaState::from_acc_map(validator_account, 1);

    let mut circuit_account_tree = CircuitAccountTree::new(models::params::account_tree_depth());
    for (id, account) in plasma_state.get_accounts() {
        circuit_account_tree.insert(id, CircuitAccount::from(account))
    }

    (plasma_state, circuit_account_tree)
}
