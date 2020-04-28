// External deps
use bigdecimal::BigDecimal;
// Workspace deps
use models::node::{operations::DepositOp, Deposit};
// Local deps
use crate::witness::{
    deposit::DepositWitness,
    tests::test_utils::{check_circuit, PlasmaStateGenerator, WitnessTestAccount, FEE_ACCOUNT_ID},
    utils::WitnessBuilder,
};

/// Checks that deposit can be applied to a new account.
/// Here we generate an empty PlasmaState (with no accounts), and make a deposit to a new account.
#[test]
#[ignore]
fn test_deposit_in_empty_leaf() {
    // Input data.
    let accounts = &[];
    let account = WitnessTestAccount::new_empty(1); // Will not be included into PlasmaState
    let deposit_op = DepositOp {
        priority_op: Deposit {
            from: account.account.address,
            token: 0,
            amount: BigDecimal::from(1),
            to: account.account.address,
        },
        account_id: account.id,
    };

    // Initialize Plasma and WitnessBuilder.
    let (mut plasma_state, mut circuit_account_tree) = PlasmaStateGenerator::generate(accounts);
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, FEE_ACCOUNT_ID, 1);

    // Apply op on plasma
    plasma_state.apply_deposit_op(&deposit_op);

    // Apply op on circuit
    let deposit_witness = DepositWitness::apply_tx(&mut witness_accum.account_tree, &deposit_op);
    let deposit_operations = deposit_witness.calculate_operations();
    let pub_data_from_witness = deposit_witness.get_pubdata();

    witness_accum.add_operation_with_pubdata(deposit_operations, pub_data_from_witness);
    witness_accum.collect_fees(&Vec::new());
    witness_accum.calculate_pubdata_commitment();

    assert_eq!(
        plasma_state.root_hash(),
        witness_accum
            .root_after_fees
            .expect("witness accum after root hash empty"),
        "root hash in state keeper and witness generation code mismatch"
    );

    check_circuit(witness_accum.into_circuit_instance());
}

/// Checks that deposit can be applied to an existing account.
/// Here we generate a PlasmaState with one account, and make a deposit to this account.
#[test]
#[ignore]
fn test_deposit_existing_account() {
    // Input data.
    let accounts = vec![WitnessTestAccount::new_empty(1)];
    let account = &accounts[0];
    let deposit_op = DepositOp {
        priority_op: Deposit {
            from: account.account.address,
            token: 0,
            amount: BigDecimal::from(1),
            to: account.account.address,
        },
        account_id: account.id,
    };

    // Initialize Plasma and WitnessBuilder.
    let (mut plasma_state, mut circuit_account_tree) = PlasmaStateGenerator::generate(&accounts);
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, FEE_ACCOUNT_ID, 1);

    // Apply op on plasma
    plasma_state.apply_deposit_op(&deposit_op);

    // Apply op on circuit
    let deposit_witness = DepositWitness::apply_tx(&mut witness_accum.account_tree, &deposit_op);
    let deposit_operations = deposit_witness.calculate_operations();
    let pub_data_from_witness = deposit_witness.get_pubdata();

    witness_accum.add_operation_with_pubdata(deposit_operations, pub_data_from_witness);
    witness_accum.collect_fees(&Vec::new());
    witness_accum.calculate_pubdata_commitment();

    assert_eq!(
        plasma_state.root_hash(),
        witness_accum
            .root_after_fees
            .expect("witness accum after root hash empty"),
        "root hash in state keeper and witness generation code mismatch"
    );

    check_circuit(witness_accum.into_circuit_instance());
}
