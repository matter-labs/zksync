// External deps
use num::BigUint;
// Workspace deps
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
use zksync_state::{
    handler::TxHandler,
    state::{CollectedFee, ZkSyncState},
};
use zksync_types::{ForcedExit, ForcedExitOp};
// Local deps
use crate::witness::{
    forced_exit::ForcedExitWitness,
    tests::test_utils::{
        corrupted_input_test_scenario, generic_test_scenario, incorrect_op_test_scenario,
        WitnessTestAccount,
    },
    utils::SigDataInput,
};

/// Basic check for execution of `ForcedExit` operation in circuit.
/// Here we create two accounts, the second one has no signing key set, and it is forced to exit by the first account.
#[test]
#[ignore]
fn test_forced_exit_success() {
    // Test vector of (withdraw_amount, fee_amount).
    let test_vector = vec![(7u64, 3u64), (1, 1), (10000, 1), (1, 10000)];

    for (withdraw_amount, fee_amount) in test_vector {
        // Input data.
        let mut accounts = vec![
            WitnessTestAccount::new(1, fee_amount),
            WitnessTestAccount::new(2, withdraw_amount),
        ];
        // Remove pubkey hash from the target account.
        accounts[1].set_empty_pubkey_hash();

        let (account_from, account_to) = (&accounts[0], &accounts[1]);
        let forced_exit_op = ForcedExitOp {
            tx: account_from.zksync_account.sign_forced_exit(
                0,
                BigUint::from(fee_amount),
                &account_to.account.address,
                None,
                true,
            ),
            target_account_id: account_to.id,
            withdraw_amount: Some(BigUint::from(withdraw_amount).into()),
        };

        // Additional data required for performing the operation.
        let input = SigDataInput::from_forced_exit_op(&forced_exit_op)
            .expect("SigDataInput creation failed");

        generic_test_scenario::<ForcedExitWitness<Bn256>, _>(
            &accounts,
            forced_exit_op,
            input,
            |plasma_state, op| {
                let fee = <ZkSyncState as TxHandler<ForcedExit>>::apply_op(plasma_state, &op)
                    .expect("ForcedExit failed")
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
    let fee_amount = 1;
    let withdraw_amount = 100;
    let mut accounts = vec![
        WitnessTestAccount::new(1, fee_amount),
        WitnessTestAccount::new(2, withdraw_amount),
    ];
    // Remove pubkey hash from the target account.
    accounts[1].set_empty_pubkey_hash();

    let (account_from, account_to) = (&accounts[0], &accounts[1]);
    let forced_exit_op = ForcedExitOp {
        tx: account_from.zksync_account.sign_forced_exit(
            0,
            BigUint::from(fee_amount),
            &account_to.account.address,
            None,
            true,
        ),
        target_account_id: account_to.id,
        withdraw_amount: Some(BigUint::from(withdraw_amount).into()),
    };

    // Additional data required for performing the operation.
    let input =
        SigDataInput::from_forced_exit_op(&forced_exit_op).expect("SigDataInput creation failed");

    // Test vector with values corrupted one by one.
    let test_vector = input.corrupted_variations();

    for input in test_vector {
        corrupted_input_test_scenario::<ForcedExitWitness<Bn256>, _>(
            &accounts,
            forced_exit_op.clone(),
            input,
            EXPECTED_PANIC_MSG,
            |plasma_state, op| {
                let fee = <ZkSyncState as TxHandler<ForcedExit>>::apply_op(plasma_state, &op)
                    .expect("Operation failed")
                    .0
                    .unwrap();
                vec![fee]
            },
        );
    }
}

/// Checks that executing a forced exit operation with incorrect
/// data (target address) results in an error.
#[test]
#[ignore]
fn test_incorrect_target() {
    const TOKEN_ID: u16 = 0;
    const WITHDRAW_AMOUNT: u64 = 7;
    const FEE_AMOUNT: u64 = 3;

    // Operation is not valid, since target account does not exist.
    const ERR_MSG: &str = "op_valid is true/enforce equal to one";

    let accounts = vec![WitnessTestAccount::new(1, FEE_AMOUNT)];

    let incorrect_account = WitnessTestAccount::new_empty(2);

    let (account_from, account_to) = (&accounts[0], &incorrect_account);
    let forced_exit_op = ForcedExitOp {
        tx: account_from.zksync_account.sign_forced_exit(
            0,
            BigUint::from(FEE_AMOUNT),
            &account_to.account.address,
            None,
            true,
        ),
        target_account_id: account_to.id,
        withdraw_amount: Some(BigUint::from(WITHDRAW_AMOUNT).into()),
    };

    // Additional data required for performing the operation.
    let input =
        SigDataInput::from_forced_exit_op(&forced_exit_op).expect("SigDataInput creation failed");

    incorrect_op_test_scenario::<ForcedExitWitness<Bn256>, _>(
        &accounts,
        forced_exit_op,
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

/// Checks that executing a transfer operation with incorrect
/// data (target account has signing key set) results in an error.
#[test]
#[ignore]
fn test_target_has_key_set() {
    const TOKEN_ID: u16 = 0;
    const FEE_AMOUNT: u64 = 3;
    const WITHDRAW_AMOUNT: u64 = 100;

    // Operation is not valid, since account has signing key set.
    const ERR_MSG: &str = "op_valid is true/enforce equal to one";

    // Input data: we DO NOT reset the signing key for the second account.
    let accounts = vec![
        WitnessTestAccount::new(1, FEE_AMOUNT),
        WitnessTestAccount::new(2, WITHDRAW_AMOUNT),
    ];

    let (account_from, account_to) = (&accounts[0], &accounts[1]);
    let forced_exit_op = ForcedExitOp {
        tx: account_from.zksync_account.sign_forced_exit(
            0,
            BigUint::from(FEE_AMOUNT),
            &account_to.account.address,
            None,
            true,
        ),
        target_account_id: account_to.id,
        withdraw_amount: Some(BigUint::from(WITHDRAW_AMOUNT).into()),
    };

    // Additional data required for performing the operation.
    let input =
        SigDataInput::from_forced_exit_op(&forced_exit_op).expect("SigDataInput creation failed");

    incorrect_op_test_scenario::<ForcedExitWitness<Bn256>, _>(
        &accounts,
        forced_exit_op,
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

/// Checks that executing a forced exit operation with incorrect
/// data (insufficient funds for fees) results in an error.
#[test]
#[ignore]
fn test_not_enough_fees() {
    const TOKEN_ID: u16 = 0;
    const FEE_AMOUNT: u64 = 3;
    const WITHDRAW_AMOUNT: u64 = 100;

    // Balance check should fail.
    // "balance-fee bits" is message for subtraction check in circuit.
    // For details see `circuit.rs`.
    const ERR_MSG: &str = "balance-fee bits";

    // Input data: we DO NOT reset the signing key for the second account.
    let accounts = vec![
        WitnessTestAccount::new(1, 0u64), // Note that initiator account has no enough funds to cover fees.
        WitnessTestAccount::new(2, WITHDRAW_AMOUNT),
    ];

    let (account_from, account_to) = (&accounts[0], &accounts[1]);
    let forced_exit_op = ForcedExitOp {
        tx: account_from.zksync_account.sign_forced_exit(
            0,
            BigUint::from(FEE_AMOUNT),
            &account_to.account.address,
            None,
            true,
        ),
        target_account_id: account_to.id,
        withdraw_amount: Some(BigUint::from(WITHDRAW_AMOUNT).into()),
    };

    // Additional data required for performing the operation.
    let input =
        SigDataInput::from_forced_exit_op(&forced_exit_op).expect("SigDataInput creation failed");

    incorrect_op_test_scenario::<ForcedExitWitness<Bn256>, _>(
        &accounts,
        forced_exit_op,
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

/// Checks that executing a forced exit operation with incorrect
/// data (insufficient target balance) results in an error.
#[test]
#[ignore]
fn test_not_enough_balance() {
    const TOKEN_ID: u16 = 0;
    const FEE_AMOUNT: u64 = 3;
    const WITHDRAW_AMOUNT: u64 = 100;

    // Balance is not equal to the withdrawals amount.
    // For details see `circuit.rs`.
    const ERR_MSG: &str = "op_valid is true/enforce equal to one";

    // Input data: we DO NOT reset the signing key for the second account.
    let accounts = vec![
        WitnessTestAccount::new(1, FEE_AMOUNT),
        WitnessTestAccount::new(2, 0u64), // Note that target account has no enough funds for withdrawal.
    ];

    let (account_from, account_to) = (&accounts[0], &accounts[1]);
    let forced_exit_op = ForcedExitOp {
        tx: account_from.zksync_account.sign_forced_exit(
            0,
            BigUint::from(FEE_AMOUNT),
            &account_to.account.address,
            None,
            true,
        ),
        target_account_id: account_to.id,
        withdraw_amount: Some(BigUint::from(WITHDRAW_AMOUNT).into()),
    };

    // Additional data required for performing the operation.
    let input =
        SigDataInput::from_forced_exit_op(&forced_exit_op).expect("SigDataInput creation failed");

    incorrect_op_test_scenario::<ForcedExitWitness<Bn256>, _>(
        &accounts,
        forced_exit_op,
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

/// Checks that executing a forced exit operation with incorrect
/// data (target balance is not equal to the amount of withdrawal) results in an error.
#[test]
#[ignore]
fn test_not_exact_withdrawal_amount() {
    const TOKEN_ID: u16 = 0;
    const FEE_AMOUNT: u64 = 3;
    const ACCOUNT_BALANCE: u64 = FEE_AMOUNT * 2;
    const WITHDRAW_AMOUNT: u64 = 100;

    // Balance is not equal to the withdrawals amount.
    // For details see `circuit.rs`.
    const ERR_MSG: &str = "op_valid is true/enforce equal to one";

    // Input data: we DO NOT reset the signing key for the second account.
    let accounts = vec![
        WitnessTestAccount::new(1, FEE_AMOUNT),
        WitnessTestAccount::new(2, ACCOUNT_BALANCE), // Note that target account has more funds than `WITHDRAW_AMOUNT`.
    ];

    let (account_from, account_to) = (&accounts[0], &accounts[1]);
    let forced_exit_op = ForcedExitOp {
        tx: account_from.zksync_account.sign_forced_exit(
            0,
            BigUint::from(FEE_AMOUNT),
            &account_to.account.address,
            None,
            true,
        ),
        target_account_id: account_to.id,
        withdraw_amount: Some(BigUint::from(WITHDRAW_AMOUNT).into()),
    };

    // Additional data required for performing the operation.
    let input =
        SigDataInput::from_forced_exit_op(&forced_exit_op).expect("SigDataInput creation failed");

    incorrect_op_test_scenario::<ForcedExitWitness<Bn256>, _>(
        &accounts,
        forced_exit_op,
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
