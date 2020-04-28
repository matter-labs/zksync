// External deps
use bigdecimal::BigDecimal;
// Workspace deps
use models::node::{operations::FullExitOp, FullExit};
// Local deps
use crate::witness::{
    full_exit::FullExitWitness,
    tests::test_utils::{check_circuit, PlasmaStateGenerator, WitnessTestAccount, FEE_ACCOUNT_ID},
    utils::WitnessBuilder,
};

/// Checks that `FullExit` can be applied to an existing account.
/// Here we generate a PlasmaState with one account (which has some funds), and
/// apply a `FullExit` to this account.
#[test]
#[ignore]
fn test_full_exit_success() {
    // Input data.
    let accounts = vec![WitnessTestAccount::new(1, 10)];
    let account = &accounts[0];
    let full_exit_op = FullExitOp {
        priority_op: FullExit {
            account_id: account.id,
            eth_address: account.account.address,
            token: 0,
        },
        withdraw_amount: Some(BigDecimal::from(10)),
    };

    // Initialize Plasma and WitnessBuilder.
    let (mut plasma_state, mut circuit_account_tree) = PlasmaStateGenerator::generate(&accounts);
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, FEE_ACCOUNT_ID, 1);

    // Apply op on plasma
    plasma_state.apply_full_exit_op(&full_exit_op);

    // Apply op on circuit
    let full_exit_witness =
        FullExitWitness::apply_tx(&mut witness_accum.account_tree, &full_exit_op, true);
    let full_exit_operations = full_exit_witness.calculate_operations();
    let pubdata_from_witness = full_exit_witness.get_pubdata();

    witness_accum.add_operation_with_pubdata(full_exit_operations, pubdata_from_witness);
    witness_accum.collect_fees(&[]);
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

#[test]
#[ignore]
fn test_full_exit_failure_no_account_in_tree() {
    // Input data.
    let accounts = &[];
    let account = WitnessTestAccount::new_empty(1); // Will not be included into PlasmaState
    let full_exit_op = FullExitOp {
        priority_op: FullExit {
            account_id: account.id,
            eth_address: account.account.address,
            token: 0,
        },
        withdraw_amount: None,
    };

    // Initialize Plasma and WitnessBuilder.
    let (mut plasma_state, mut circuit_account_tree) = PlasmaStateGenerator::generate(accounts);
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, FEE_ACCOUNT_ID, 1);

    // Apply op on plasma
    plasma_state.apply_full_exit_op(&full_exit_op);

    // Apply op on circuit
    let full_exit_witness =
        FullExitWitness::apply_tx(&mut witness_accum.account_tree, &full_exit_op, false);
    let full_exit_operations = full_exit_witness.calculate_operations();
    let pubdata_from_witness = full_exit_witness.get_pubdata();

    witness_accum.add_operation_with_pubdata(full_exit_operations, pubdata_from_witness);
    witness_accum.collect_fees(&[]);
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
