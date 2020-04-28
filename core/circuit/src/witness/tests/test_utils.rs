// External deps
use bigdecimal::BigDecimal;
use crypto_exports::franklin_crypto::{bellman::Circuit, circuit::test::TestConstraintSystem};
// Workspace deps
use models::{
    circuit::{account::CircuitAccount, CircuitAccountTree},
    node::{Account, AccountId, AccountMap, Address, Engine},
};
use plasma::state::{CollectedFee, PlasmaState};
use testkit::zksync_account::ZksyncAccount;
// Local deps
use crate::{circuit::FranklinCircuit, witness::Witness};

// Public re-exports
pub use crate::witness::utils::WitnessBuilder;

pub const FEE_ACCOUNT_ID: u32 = 0;

/// Verifies that circuit has no unsatisfied constraints, and panics otherwise.
pub fn check_circuit(circuit: FranklinCircuit<Engine>) {
    let mut cs = TestConstraintSystem::<Engine>::new();
    circuit.synthesize(&mut cs).unwrap();

    println!("unconstrained: {}", cs.find_unconstrained());
    println!("number of constraints {}", cs.num_constraints());
    if let Some(err) = cs.which_is_unsatisfied() {
        panic!("ERROR satisfying in {}", err);
    }
}

/// Helper structure to generate `PlasmaState` and `CircuitAccountTree`.
#[derive(Debug)]
pub struct PlasmaStateGenerator;

impl PlasmaStateGenerator {
    fn create_state(accounts: AccountMap) -> (PlasmaState, CircuitAccountTree) {
        let plasma_state = PlasmaState::from_acc_map(accounts, 1);

        let mut circuit_account_tree =
            CircuitAccountTree::new(models::params::account_tree_depth());
        for (id, account) in plasma_state.get_accounts() {
            circuit_account_tree.insert(id, CircuitAccount::from(account))
        }

        (plasma_state, circuit_account_tree)
    }

    pub fn generate(accounts: &[WitnessTestAccount]) -> (PlasmaState, CircuitAccountTree) {
        let accounts: Vec<_> = accounts
            .iter()
            .map(|acc| (acc.id, acc.account.clone()))
            .collect();

        if accounts.iter().any(|(id, _)| *id == FEE_ACCOUNT_ID) {
            panic!("AccountId {} is an existing fee account", FEE_ACCOUNT_ID);
        }

        let validator_accounts = std::iter::once((
            FEE_ACCOUNT_ID,
            Account::default_with_address(&Address::default()),
        ))
        .chain(accounts)
        .collect();

        Self::create_state(validator_accounts)
    }
}

/// A helper structure for witness tests which contains both testkit
/// zkSync account and an actual zkSync account.
#[derive(Debug)]
pub struct WitnessTestAccount {
    pub zksync_account: ZksyncAccount,
    pub id: AccountId,
    pub account: Account,
}

impl WitnessTestAccount {
    pub fn new(id: AccountId, balance: u64) -> Self {
        let zksync_account = ZksyncAccount::rand();
        let account = {
            let mut account = Account::default_with_address(&zksync_account.address);
            account.add_balance(0, &BigDecimal::from(balance));
            account.pub_key_hash = zksync_account.pubkey_hash.clone();
            account
        };

        Self {
            zksync_account,
            id,
            account,
        }
    }

    pub fn new_empty(id: AccountId) -> Self {
        Self::new(id, 0)
    }
}

/// Generic test scenario does the following:
/// - Initializes plasma state
/// - Applies the provided operation on plasma
/// - Applies the provided operation on circuit
/// - Verifies that root hashes in plasma and circuit match
/// - Verifies that there are no unsatisfied constraints in the circuit.
pub fn generic_test_scenario<W, F>(
    accounts: &[WitnessTestAccount],
    op: W::OperationType,
    input: W::CalculateOpsInput,
    apply_op_on_plasma: F,
) where
    W: Witness,
    F: FnOnce(&mut PlasmaState, &W::OperationType) -> Vec<CollectedFee>,
{
    // Initialize Plasma and WitnessBuilder.
    let (mut plasma_state, mut circuit_account_tree) = PlasmaStateGenerator::generate(&accounts);
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, FEE_ACCOUNT_ID, 1);

    // Apply op on plasma
    let fees = apply_op_on_plasma(&mut plasma_state, &op);
    plasma_state.collect_fee(&fees, FEE_ACCOUNT_ID);

    // Apply op on circuit
    let witness = W::apply_tx(&mut witness_accum.account_tree, &op);
    let circuit_operations = witness.calculate_operations(input);
    let pub_data_from_witness = witness.get_pubdata();

    // Prepare circuit
    witness_accum.add_operation_with_pubdata(circuit_operations, pub_data_from_witness);
    witness_accum.collect_fees(&fees);
    witness_accum.calculate_pubdata_commitment();

    // Check that root hashes match
    assert_eq!(
        plasma_state.root_hash(),
        witness_accum
            .root_after_fees
            .expect("witness accum after root hash empty"),
        "root hash in state keeper and witness generation code mismatch"
    );

    // Verify that there are no unsatisfied constraints
    check_circuit(witness_accum.into_circuit_instance());
}
