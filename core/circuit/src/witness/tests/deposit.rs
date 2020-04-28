// External deps
use bigdecimal::BigDecimal;
use crypto_exports::franklin_crypto::bellman::pairing::bn256::Bn256;
// Workspace deps
use models::node::{operations::DepositOp, Deposit};
// Local deps
use crate::witness::{
    deposit::DepositWitness,
    tests::test_utils::{generic_test_scenario, WitnessTestAccount},
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

    generic_test_scenario::<DepositWitness<Bn256>, _>(
        accounts,
        deposit_op,
        (),
        |plasma_state, op| {
            plasma_state.apply_deposit_op(op);
            vec![]
        },
    );
}

/// Checks that deposit can be applied to an existing account.
/// Here we generate a PlasmaState with one account, and make a deposit to this account.
#[test]
#[ignore]
fn test_deposit_existing_account() {
    // Data for building a test vector: tuples of (token_id, amount).
    let test_vector = vec![
        (0, 1),             // Small deposit in ETH.
        (0, 0),             // 0 deposit in ETH.
        (0, std::u64::MAX), // Big amount in ETH.
        (2, 1),             // Non-ETH token.
    ];

    for (token_id, token_amount) in test_vector {
        // Input data.
        let accounts = vec![WitnessTestAccount::new_empty(1)];
        let account = &accounts[0];
        let deposit_op = DepositOp {
            priority_op: Deposit {
                from: account.account.address,
                token: token_id,
                amount: BigDecimal::from(token_amount),
                to: account.account.address,
            },
            account_id: account.id,
        };

        generic_test_scenario::<DepositWitness<Bn256>, _>(
            &accounts,
            deposit_op,
            (),
            |plasma_state, op| {
                plasma_state.apply_deposit_op(op);
                vec![]
            },
        );
    }
}
