// External deps
use num::BigUint;
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
// Workspace deps
use zksync_state::{handler::TxHandler, state::ZkSyncState};
use zksync_types::{operations::DepositOp, Deposit};
// Local deps
use crate::witness::{
    deposit::DepositWitness,
    tests::test_utils::{generic_test_scenario, WitnessTestAccount},
};

/// Checks that deposit can be applied to a new account.
/// Here we generate an empty ZkSyncState (with no accounts), and make a deposit to a new account.
#[test]
#[ignore]
fn test_deposit_in_empty_leaf() {
    // Input data.
    let accounts = &[];
    let account = WitnessTestAccount::new_empty(1); // Will not be included into ZkSyncState
    let deposit_op = DepositOp {
        priority_op: Deposit {
            from: account.account.address,
            token: 0,
            amount: BigUint::from(1u32),
            to: account.account.address,
        },
        account_id: account.id,
    };

    generic_test_scenario::<DepositWitness<Bn256>, _>(
        accounts,
        deposit_op,
        (),
        |plasma_state, op| {
            <ZkSyncState as TxHandler<Deposit>>::apply_op(plasma_state, op)
                .expect("Deposit failed");
            vec![]
        },
    );
}

/// Checks that deposit can be applied to an existing account.
/// Here we generate a ZkSyncState with one account, and make a deposit to this account.
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
                amount: BigUint::from(token_amount),
                to: account.account.address,
            },
            account_id: account.id,
        };

        generic_test_scenario::<DepositWitness<Bn256>, _>(
            &accounts,
            deposit_op,
            (),
            |plasma_state, op| {
                <ZkSyncState as TxHandler<Deposit>>::apply_op(plasma_state, op)
                    .expect("Deposit failed");
                vec![]
            },
        );
    }
}

/// Checks that executing a deposit operation with incorrect
/// data results in an error.
#[test]
#[ignore]
#[should_panic(expected = "assertion failed: (acc.address == deposit.address)")]
fn test_incorrect_deposit_address() {
    const TOKEN_ID: u16 = 0;
    const TOKEN_AMOUNT: u32 = 100;

    let accounts = vec![WitnessTestAccount::new_empty(1)];
    let account = &accounts[0];

    // Create a deposit operation with an incorrect recipient address.
    let deposit_op = DepositOp {
        priority_op: Deposit {
            from: account.account.address,
            token: TOKEN_ID,
            amount: BigUint::from(TOKEN_AMOUNT),
            to: Default::default(),
        },
        account_id: account.id,
    };

    // Attempt to apply incorrect operation should result in an assertion failure.
    generic_test_scenario::<DepositWitness<Bn256>, _>(
        &accounts,
        deposit_op,
        (),
        |plasma_state, op| {
            <ZkSyncState as TxHandler<Deposit>>::apply_op(plasma_state, op)
                .expect("Deposit failed");
            vec![]
        },
    );
}
