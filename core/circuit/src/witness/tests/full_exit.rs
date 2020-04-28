// External deps
use bigdecimal::BigDecimal;
use crypto_exports::franklin_crypto::bellman::pairing::bn256::Bn256;
// Workspace deps
use models::node::{operations::FullExitOp, FullExit};
// Local deps
use crate::witness::{
    full_exit::FullExitWitness,
    tests::test_utils::{generic_test_scenario, WitnessTestAccount},
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
    let success = true;

    generic_test_scenario::<FullExitWitness<Bn256>, _>(
        &accounts,
        (full_exit_op, success),
        (),
        |plasma_state, op| {
            plasma_state.apply_full_exit_op(&op.0);
            vec![]
        },
    );
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
    let success = false;

    generic_test_scenario::<FullExitWitness<Bn256>, _>(
        accounts,
        (full_exit_op, success),
        (),
        |plasma_state, op| {
            plasma_state.apply_full_exit_op(&op.0);
            vec![]
        },
    );
}
