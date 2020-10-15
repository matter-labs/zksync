// External deps
use num::BigUint;
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
// Workspace deps
use zksync_state::state::CollectedFee;
use zksync_state::{handler::TxHandler, state::ZkSyncState};
use zksync_types::{operations::WithdrawOp, tx::Withdraw, Address};
// Local deps
use crate::witness::{
    tests::test_utils::{
        corrupted_input_test_scenario, generic_test_scenario, incorrect_op_test_scenario,
        WitnessTestAccount,
    },
    utils::SigDataInput,
    withdraw::WithdrawWitness,
};

#[test]
#[ignore]
fn test_withdraw() {
    // Test vector of (initial_balance, transfer_amount, fee_amount).
    let test_vector = vec![
        (10u64, 7u64, 3u64),       // Basic transfer
        (0, 0, 0),                 // Zero transfer
        (std::u64::MAX, 1, 1),     // Small transfer from rich account,
        (std::u64::MAX, 10000, 1), // Big transfer from rich account (too big values can't be used, since they're not packable),
        (std::u64::MAX, 1, 10000), // Very big fee
    ];

    for (initial_balance, transfer_amount, fee_amount) in test_vector {
        // Input data.
        let accounts = vec![WitnessTestAccount::new(1, initial_balance)];
        let account = &accounts[0];
        let withdraw_op = WithdrawOp {
            tx: account
                .zksync_account
                .sign_withdraw(
                    0,
                    "",
                    BigUint::from(transfer_amount),
                    BigUint::from(fee_amount),
                    &Address::zero(),
                    None,
                    true,
                )
                .0,
            account_id: account.id,
        };

        // Additional data required for performing the operation.
        let input =
            SigDataInput::from_withdraw_op(&withdraw_op).expect("SigDataInput creation failed");

        generic_test_scenario::<WithdrawWitness<Bn256>, _>(
            &accounts,
            withdraw_op,
            input,
            |plasma_state, op| {
                let fee = <ZkSyncState as TxHandler<Withdraw>>::apply_op(plasma_state, &op)
                    .expect("Operation failed")
                    .0
                    .unwrap();
                vec![fee]
            },
        );
    }
}

/// Checks that corrupted signature data leads to unsatisfied constraints in circuit.
#[test]
#[ignore]
fn corrupted_ops_input() {
    // Incorrect signature data will lead to `op_valid` constraint failure.
    // See `circuit.rs` for details.
    const EXPECTED_PANIC_MSG: &str = "op_valid is true";

    // Legit input data.
    let accounts = vec![WitnessTestAccount::new(1, 10)];
    let account = &accounts[0];
    let withdraw_op = WithdrawOp {
        tx: account
            .zksync_account
            .sign_withdraw(
                0,
                "",
                BigUint::from(7u64),
                BigUint::from(3u64),
                &Address::zero(),
                None,
                true,
            )
            .0,
        account_id: account.id,
    };

    // Additional data required for performing the operation.
    let input = SigDataInput::from_withdraw_op(&withdraw_op).expect("SigDataInput creation failed");

    // Test vector with values corrupted one by one.
    let test_vector = input.corrupted_variations();

    for input in test_vector {
        corrupted_input_test_scenario::<WithdrawWitness<Bn256>, _>(
            &accounts,
            withdraw_op.clone(),
            input,
            EXPECTED_PANIC_MSG,
            |plasma_state, op| {
                let fee = <ZkSyncState as TxHandler<Withdraw>>::apply_op(plasma_state, &op)
                    .expect("Operation failed")
                    .0
                    .unwrap();
                vec![fee]
            },
        );
    }
}

/// Checks that executing a withdraw operation with incorrect
/// data (account `from` ID) results in an error.
#[test]
#[ignore]
fn test_incorrect_withdraw_account_from() {
    const TOKEN_ID: u16 = 0;
    const INITIAL_BALANCE: u64 = 10;
    const TOKEN_AMOUNT: u64 = 7;
    const FEE_AMOUNT: u64 = 3;

    // Operation is not valid, since `from` ID is different from the tx body.
    const ERR_MSG: &str = "op_valid is true/enforce equal to one";

    let incorrect_from_account = WitnessTestAccount::new(3, INITIAL_BALANCE);

    // Input data: transaction is signed by an incorrect account (address of account
    // and ID of the `from` accounts differ).
    let accounts = vec![WitnessTestAccount::new(1, INITIAL_BALANCE)];
    let account_from = &accounts[0];
    let withdraw_op = WithdrawOp {
        tx: incorrect_from_account
            .zksync_account
            .sign_withdraw(
                TOKEN_ID,
                "",
                BigUint::from(TOKEN_AMOUNT),
                BigUint::from(FEE_AMOUNT),
                &Address::zero(),
                None,
                true,
            )
            .0,
        account_id: account_from.id,
    };

    let input = SigDataInput::from_withdraw_op(&withdraw_op).expect("SigDataInput creation failed");

    incorrect_op_test_scenario::<WithdrawWitness<Bn256>, _>(
        &accounts,
        withdraw_op,
        input,
        ERR_MSG,
        || {
            vec![CollectedFee {
                token: TOKEN_ID,
                amount: FEE_AMOUNT.into(),
            }]
        },
    );
}

/// Checks that executing a withdraw operation with incorrect
/// data (insufficient funds) results in an error.
#[test]
#[ignore]
fn test_incorrect_withdraw_amount() {
    const TOKEN_ID: u16 = 0;
    // Balance check should fail.
    // "balance-fee bits" is message for subtraction check in circuit.
    // For details see `circuit.rs`.
    const ERR_MSG: &str = "balance-fee bits";

    // Test vector of (initial_balance, transfer_amount, fee_amount).
    let test_vector = vec![
        (10u64, 15u64, 0u64), // Withdraw too big
        (10, 7, 4),           // Fee too big
        (0, 1, 1),            // Withdraw from 0 balance
    ];

    for (initial_balance, transfer_amount, fee_amount) in test_vector {
        // Input data: account does not have enough funds.
        let accounts = vec![WitnessTestAccount::new(1, initial_balance)];
        let account_from = &accounts[0];
        let withdraw_op = WithdrawOp {
            tx: account_from
                .zksync_account
                .sign_withdraw(
                    TOKEN_ID,
                    "",
                    BigUint::from(transfer_amount),
                    BigUint::from(fee_amount),
                    &Address::zero(),
                    None,
                    true,
                )
                .0,
            account_id: account_from.id,
        };

        let input =
            SigDataInput::from_withdraw_op(&withdraw_op).expect("SigDataInput creation failed");

        incorrect_op_test_scenario::<WithdrawWitness<Bn256>, _>(
            &accounts,
            withdraw_op,
            input,
            ERR_MSG,
            || {
                vec![CollectedFee {
                    token: TOKEN_ID,
                    amount: fee_amount.into(),
                }]
            },
        );
    }
}

/// Checks that even if there are two accounts with the same keys in the state,
/// one account cannot authorize the withdraw from its duplicate.
#[test]
#[ignore]
fn test_withdraw_replay() {
    const TOKEN_ID: u16 = 0;
    const INITIAL_BALANCE: u64 = 10;
    const TOKEN_AMOUNT: u64 = 7;
    const FEE_AMOUNT: u64 = 3;

    // Operation is not valid, since the balance is already transferred from account
    // with the same private key.
    const ERR_MSG: &str = "op_valid is true/enforce equal to one";

    let account_base = WitnessTestAccount::new(1, INITIAL_BALANCE);
    // Create a copy of the base account with the same keys.
    let mut account_copy = WitnessTestAccount::new_empty(2);
    account_copy.account = account_base.account.clone();

    let account_to = WitnessTestAccount::new_empty(3); // Will not be included into state.

    // Input data
    let accounts = vec![account_base, account_copy];

    let (account_from, account_copy) = (&accounts[0], &accounts[1]);

    // Create the transfer_op, and set the `from` ID to the duplicate account ID.
    // Despite that both account and duplicate account have the same keys, withdraw
    // operation contains the account ID, and transaction should fail.
    let withdraw_op = WithdrawOp {
        tx: account_from
            .zksync_account
            .sign_withdraw(
                TOKEN_ID,
                "",
                BigUint::from(TOKEN_AMOUNT),
                BigUint::from(FEE_AMOUNT),
                &account_to.account.address,
                None,
                true,
            )
            .0,
        account_id: account_copy.id,
    };

    let input = SigDataInput::from_withdraw_op(&withdraw_op).expect("SigDataInput creation failed");

    incorrect_op_test_scenario::<WithdrawWitness<Bn256>, _>(
        &accounts,
        withdraw_op,
        input,
        ERR_MSG,
        || {
            vec![CollectedFee {
                token: TOKEN_ID,
                amount: FEE_AMOUNT.into(),
            }]
        },
    );
}
