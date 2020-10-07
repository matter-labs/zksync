// External deps
use num::BigUint;
use zksync_crypto::franklin_crypto::{
    bellman::{pairing::ff::PrimeField, Circuit},
    circuit::test::TestConstraintSystem,
};
// Workspace deps
use zksync_crypto::circuit::{account::CircuitAccount, CircuitAccountTree};
use zksync_crypto::{Engine, Fr};
use zksync_state::state::{CollectedFee, ZkSyncState};
use zksync_test_account::ZkSyncAccount;
use zksync_types::{Account, AccountId, AccountMap, Address};
// Local deps
use crate::{circuit::ZkSyncCircuit, witness::Witness};

// Public re-exports
pub use crate::witness::utils::WitnessBuilder;

pub const FEE_ACCOUNT_ID: u32 = 0;

/// Verifies that circuit has no unsatisfied constraints, and returns an error otherwise.
pub fn check_circuit_non_panicking(circuit: ZkSyncCircuit<Engine>) -> Result<(), String> {
    let mut cs = TestConstraintSystem::<Engine>::new();
    circuit.synthesize(&mut cs).unwrap();

    println!("unconstrained: {}", cs.find_unconstrained());
    println!("number of constraints {}", cs.num_constraints());
    if let Some(err) = cs.which_is_unsatisfied() {
        Err(err.into())
    } else {
        Ok(())
    }
}

/// Verifies that circuit has no unsatisfied constraints, and panics otherwise.
pub fn check_circuit(circuit: ZkSyncCircuit<Engine>) {
    check_circuit_non_panicking(circuit).expect("ERROR satisfying the constraints:")
}

// Provides a quasi-random non-zero `Fr` to substitute an incorrect `Fr` value.
pub fn incorrect_fr() -> Fr {
    Fr::from_str("12345").unwrap()
}

/// Helper structure to generate `ZkSyncState` and `CircuitAccountTree`.
#[derive(Debug)]
pub struct ZkSyncStateGenerator;

impl ZkSyncStateGenerator {
    fn create_state(accounts: AccountMap) -> (ZkSyncState, CircuitAccountTree) {
        let plasma_state = ZkSyncState::from_acc_map(accounts, 1);

        let mut circuit_account_tree =
            CircuitAccountTree::new(zksync_crypto::params::account_tree_depth());
        for (id, account) in plasma_state.get_accounts() {
            circuit_account_tree.insert(id, CircuitAccount::from(account))
        }

        (plasma_state, circuit_account_tree)
    }

    pub fn generate(accounts: &[WitnessTestAccount]) -> (ZkSyncState, CircuitAccountTree) {
        let accounts: Vec<_> = accounts
            .iter()
            .map(|acc| (acc.id, acc.account.clone()))
            .collect();

        let accounts = if accounts.iter().any(|(id, _)| *id == FEE_ACCOUNT_ID) {
            println!(
                "Note: AccountId {} is an existing fee account",
                FEE_ACCOUNT_ID
            );
            accounts.into_iter().collect()
        } else {
            std::iter::once((
                FEE_ACCOUNT_ID,
                Account::default_with_address(&Address::default()),
            ))
            .chain(accounts)
            .collect()
        };

        Self::create_state(accounts)
    }
}

/// A helper structure for witness tests which contains both testkit
/// zkSync account and an actual zkSync account.
#[derive(Debug)]
pub struct WitnessTestAccount {
    pub zksync_account: ZkSyncAccount,
    pub id: AccountId,
    pub account: Account,
}

impl WitnessTestAccount {
    pub fn new(id: AccountId, balance: u64) -> Self {
        let zksync_account = ZkSyncAccount::rand();
        zksync_account.set_account_id(Some(id));

        let account = {
            let mut account = Account::default_with_address(&zksync_account.address);
            account.add_balance(0, &BigUint::from(balance));
            account.pub_key_hash = zksync_account.pubkey_hash.clone();
            account
        };

        Self {
            zksync_account,
            id,
            account,
        }
    }

    pub fn set_empty_pubkey_hash(&mut self) {
        self.zksync_account.pubkey_hash = Default::default();
        self.account.pub_key_hash = Default::default();
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
    F: FnOnce(&mut ZkSyncState, &W::OperationType) -> Vec<CollectedFee>,
{
    // Initialize Plasma and WitnessBuilder.
    let (mut plasma_state, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);
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

/// Does the same operations as the `generic_test_scenario`, but assumes
/// that input for `calculate_operations` is corrupted and will lead to an error.
/// The error is caught and checked to match the provided message.
pub fn corrupted_input_test_scenario<W, F>(
    accounts: &[WitnessTestAccount],
    op: W::OperationType,
    input: W::CalculateOpsInput,
    expected_msg: &str,
    apply_op_on_plasma: F,
) where
    W: Witness,
    W::CalculateOpsInput: Clone + std::fmt::Debug,
    F: FnOnce(&mut ZkSyncState, &W::OperationType) -> Vec<CollectedFee>,
{
    // Initialize Plasma and WitnessBuilder.
    let (mut plasma_state, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, FEE_ACCOUNT_ID, 1);

    // Apply op on plasma
    let fees = apply_op_on_plasma(&mut plasma_state, &op);
    plasma_state.collect_fee(&fees, FEE_ACCOUNT_ID);

    // Apply op on circuit
    let witness = W::apply_tx(&mut witness_accum.account_tree, &op);
    let circuit_operations = witness.calculate_operations(input.clone());
    let pub_data_from_witness = witness.get_pubdata();

    // Prepare circuit
    witness_accum.add_operation_with_pubdata(circuit_operations, pub_data_from_witness);
    witness_accum.collect_fees(&fees);
    witness_accum.calculate_pubdata_commitment();

    let result = check_circuit_non_panicking(witness_accum.into_circuit_instance());

    match result {
        Ok(_) => panic!(
            "Operation did not err, but was expected to err with message '{}' \
             Provided input: {:?}",
            expected_msg, input
        ),
        Err(error_msg) => {
            assert!(
                error_msg.contains(expected_msg),
                "Code erred with unexpected message. \
                 Provided message: '{}', but expected '{}'. \
                 Provided input: {:?}",
                error_msg,
                expected_msg,
                input,
            );
        }
    }
}

/// Performs the operation on the circuit, but not on the plasma,
/// since the operation is meant to be incorrect and should result in an error.
/// The error is caught and checked to match the provided message.
pub fn incorrect_op_test_scenario<W, F>(
    accounts: &[WitnessTestAccount],
    op: W::OperationType,
    input: W::CalculateOpsInput,
    expected_msg: &str,
    collect_fees: F,
) where
    W: Witness,
    W::CalculateOpsInput: Clone + std::fmt::Debug,
    F: FnOnce() -> Vec<CollectedFee>,
{
    // Initialize WitnessBuilder.
    let (_, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, FEE_ACCOUNT_ID, 1);

    // Collect fees without actually applying the tx on plasma
    let fees = collect_fees();

    // Apply op on circuit
    let witness = W::apply_tx(&mut witness_accum.account_tree, &op);
    let circuit_operations = witness.calculate_operations(input.clone());
    let pub_data_from_witness = witness.get_pubdata();

    // Prepare circuit
    witness_accum.add_operation_with_pubdata(circuit_operations, pub_data_from_witness);
    witness_accum.collect_fees(&fees);
    witness_accum.calculate_pubdata_commitment();

    let result = check_circuit_non_panicking(witness_accum.into_circuit_instance());

    match result {
        Ok(_) => panic!(
            "Operation did not err, but was expected to err with message '{}' \
             Provided input: {:?}",
            expected_msg, input
        ),
        Err(error_msg) => {
            assert!(
                error_msg.contains(expected_msg),
                "Code erred with unexpected message. \
                 Provided message: '{}', but expected '{}'. \
                 Provided input: {:?}",
                error_msg,
                expected_msg,
                input,
            );
        }
    }
}
