// External deps
use num::BigUint;
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
// Workspace deps
use zksync_state::{
    handler::TxHandler,
    state::{CollectedFee, TransferOutcome, ZkSyncState},
};
use zksync_types::{
    operations::TransferOp,
    tx::{TimeRange, Transfer, TxSignature},
    AccountId, Nonce, TokenId,
};
// Local deps
use crate::witness::{
    tests::test_utils::{
        corrupted_input_test_scenario, generic_test_scenario, incorrect_op_test_scenario,
        WitnessTestAccount, BLOCK_TIMESTAMP,
    },
    transfer::TransferWitness,
    utils::SigDataInput,
};
use zksync_crypto::params::{number_of_processable_tokens, NFT_STORAGE_ACCOUNT_ID, NFT_TOKEN_ID};

/// Basic check for execution of `Transfer` operation in circuit.
/// Here we create two accounts and perform a transfer between them.
#[test]
#[ignore]
fn test_transfer_success() {
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
        let accounts = vec![
            WitnessTestAccount::new(AccountId(1), initial_balance),
            WitnessTestAccount::new_empty(AccountId(2)),
        ];
        let (account_from, account_to) = (&accounts[0], &accounts[1]);
        let transfer_op = TransferOp {
            tx: account_from
                .zksync_account
                .sign_transfer(
                    TokenId(0),
                    "",
                    BigUint::from(transfer_amount),
                    BigUint::from(fee_amount),
                    &account_to.account.address,
                    None,
                    true,
                    Default::default(),
                )
                .0,
            from: account_from.id,
            to: account_to.id,
        };

        // Additional data required for performing the operation.
        let input =
            SigDataInput::from_transfer_op(&transfer_op).expect("SigDataInput creation failed");

        generic_test_scenario::<TransferWitness<Bn256>, _>(
            &accounts,
            transfer_op,
            input,
            |plasma_state, op| {
                let raw_op = TransferOutcome::Transfer(op.clone());
                let fee = <ZkSyncState as TxHandler<Transfer>>::apply_op(plasma_state, &raw_op)
                    .expect("Operation failed")
                    .0
                    .unwrap();
                vec![fee]
            },
        );
    }
}

/// Basic check for execution of `Transfer` operation in circuit with old signature scheme.
/// Here we create two accounts and perform a transfer between them.
#[test]
#[ignore]
fn test_transfer_old_signature_success() {
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
        let accounts = vec![
            WitnessTestAccount::new(AccountId(1), initial_balance),
            WitnessTestAccount::new_empty(AccountId(2)),
        ];
        let (account_from, account_to) = (&accounts[0], &accounts[1]);
        let mut tx = Transfer::new(
            AccountId(1),
            account_from.zksync_account.address,
            account_to.account.address,
            TokenId(0),
            BigUint::from(transfer_amount),
            BigUint::from(fee_amount),
            Nonce(0),
            Default::default(),
            None,
        );
        tx.signature = TxSignature::sign_musig(
            &account_from.zksync_account.private_key,
            &tx.get_old_bytes(),
        );
        let transfer_op = TransferOp {
            tx,
            from: account_from.id,
            to: account_to.id,
        };

        let sign_packed = transfer_op
            .tx
            .signature
            .signature
            .serialize_packed()
            .expect("signature serialize");
        let input = SigDataInput::new(
            &sign_packed,
            &transfer_op.tx.get_old_bytes(),
            &transfer_op.tx.signature.pub_key,
        )
        .expect("input constructing fails");

        generic_test_scenario::<TransferWitness<Bn256>, _>(
            &accounts,
            transfer_op,
            input,
            |plasma_state, op| {
                let raw_op = TransferOutcome::Transfer(op.clone());
                let fee = <ZkSyncState as TxHandler<Transfer>>::apply_op(plasma_state, &raw_op)
                    .expect("Operation failed")
                    .0
                    .unwrap();
                vec![fee]
            },
        );
    }
}

/// Check for execution of `Transfer` to self works with max token id.
/// Here we create one accounts and perform a transfer to self.
#[test]
#[ignore]
fn test_transfer_to_self_max_token_id() {
    let max_token_id = TokenId(number_of_processable_tokens() as u32 - 1);
    // Input data.
    let mut account = WitnessTestAccount::new(AccountId(1), 10);
    account.account.add_balance(max_token_id, &10u32.into());
    let accounts = vec![account];
    let account = &accounts[0];
    let transfer_op = TransferOp {
        tx: account
            .zksync_account
            .sign_transfer(
                max_token_id,
                "",
                BigUint::from(7u32),
                BigUint::from(3u32),
                &account.account.address,
                None,
                true,
                Default::default(),
            )
            .0,
        from: account.id,
        to: account.id,
    };

    // Additional data required for performing the operation.
    let input = SigDataInput::from_transfer_op(&transfer_op).expect("SigDataInput creation failed");

    generic_test_scenario::<TransferWitness<Bn256>, _>(
        &accounts,
        transfer_op,
        input,
        |plasma_state, op| {
            let raw_op = TransferOutcome::Transfer(op.clone());
            let fee = <ZkSyncState as TxHandler<Transfer>>::apply_op(plasma_state, &raw_op)
                .expect("Operation failed")
                .0
                .unwrap();
            vec![fee]
        },
    );
}

/// Check for execution of `Transfer` operation with recipient same as sender in circuit.
/// Here we create one accounts and perform a transfer to self.
#[test]
#[ignore]
fn test_transfer_to_self() {
    // Input data.
    let accounts = vec![WitnessTestAccount::new(AccountId(1), 10)];
    let account = &accounts[0];
    let transfer_op = TransferOp {
        tx: account
            .zksync_account
            .sign_transfer(
                TokenId(0),
                "",
                BigUint::from(7u32),
                BigUint::from(3u32),
                &account.account.address,
                None,
                true,
                Default::default(),
            )
            .0,
        from: account.id,
        to: account.id,
    };

    // Additional data required for performing the operation.
    let input = SigDataInput::from_transfer_op(&transfer_op).expect("SigDataInput creation failed");

    generic_test_scenario::<TransferWitness<Bn256>, _>(
        &accounts,
        transfer_op,
        input,
        |plasma_state, op| {
            let raw_op = TransferOutcome::Transfer(op.clone());
            let fee = <ZkSyncState as TxHandler<Transfer>>::apply_op(plasma_state, &raw_op)
                .expect("Operation failed")
                .0
                .unwrap();
            vec![fee]
        },
    );
}

/// Checks that corrupted signature data leads to unsatisfied constraints in circuit.
#[test]
#[ignore]
fn corrupted_ops_input() {
    // Incorrect signature data will lead to `op_valid` constraint failure.
    // See `circuit.rs` for details.
    const EXPECTED_PANIC_MSG: &str = "op_valid is true";

    // Legit input data.
    let accounts = vec![WitnessTestAccount::new(AccountId(1), 10)];
    let account = &accounts[0];
    let transfer_op = TransferOp {
        tx: account
            .zksync_account
            .sign_transfer(
                TokenId(0),
                "",
                BigUint::from(7u32),
                BigUint::from(3u32),
                &account.account.address,
                None,
                true,
                Default::default(),
            )
            .0,
        from: account.id,
        to: account.id,
    };

    // Additional data required for performing the operation.
    let input = SigDataInput::from_transfer_op(&transfer_op).expect("SigDataInput creation failed");

    // Test vector with values corrupted one by one.
    let test_vector = input.corrupted_variations();

    for input in test_vector {
        corrupted_input_test_scenario::<TransferWitness<Bn256>, _, _>(
            &accounts,
            transfer_op.clone(),
            input,
            EXPECTED_PANIC_MSG,
            |plasma_state, op| {
                let raw_op = TransferOutcome::Transfer(op.clone());
                let fee = <ZkSyncState as TxHandler<Transfer>>::apply_op(plasma_state, &raw_op)
                    .expect("Operation failed")
                    .0
                    .unwrap();
                vec![fee]
            },
            |_| {},
        );
    }
}

/// Checks that executing a transfer operation with incorrect
/// data (account `from` ID) results in an error.
#[test]
#[ignore]
fn test_incorrect_transfer_account_from() {
    const TOKEN_ID: TokenId = TokenId(0);
    const INITIAL_BALANCE: u64 = 10;
    const TOKEN_AMOUNT: u64 = 7;
    const FEE_AMOUNT: u64 = 3;

    // Operation is not valid, since `from` ID is different from the tx body.
    const ERR_MSG: &str = "op_valid is true/enforce equal to one";

    let incorrect_from_account = WitnessTestAccount::new(AccountId(3), INITIAL_BALANCE);

    // Input data: transaction is signed by an incorrect account (address of account
    // and ID of the `from` accounts differ).
    let accounts = vec![
        WitnessTestAccount::new(AccountId(1), INITIAL_BALANCE),
        WitnessTestAccount::new_empty(AccountId(2)),
    ];
    let (account_from, account_to) = (&accounts[0], &accounts[1]);
    let transfer_op = TransferOp {
        tx: incorrect_from_account
            .zksync_account
            .sign_transfer(
                TOKEN_ID,
                "",
                BigUint::from(TOKEN_AMOUNT),
                BigUint::from(FEE_AMOUNT),
                &account_to.account.address,
                None,
                true,
                Default::default(),
            )
            .0,
        from: account_from.id,
        to: account_to.id,
    };

    let input = SigDataInput::from_transfer_op(&transfer_op).expect("SigDataInput creation failed");

    incorrect_op_test_scenario::<TransferWitness<Bn256>, _, _>(
        &accounts,
        transfer_op,
        input,
        ERR_MSG,
        || {
            vec![CollectedFee {
                token: TOKEN_ID,
                amount: FEE_AMOUNT.into(),
            }]
        },
        |_| {},
    );
}

/// Checks that executing a transfer operation with incorrect
/// data (account `to` ID) results in an error.
#[test]
#[ignore]
fn test_incorrect_transfer_account_to() {
    const TOKEN_ID: TokenId = TokenId(0);
    const INITIAL_BALANCE: u64 = 10;
    const TOKEN_AMOUNT: u32 = 7;
    const FEE_AMOUNT: u32 = 3;

    // Operation is not valid, since `to` ID is different from the tx body.
    const ERR_MSG: &str = "op_valid is true/enforce equal to one";

    // Input data: address of account and ID of the `to` accounts differ.
    let accounts = vec![
        WitnessTestAccount::new(AccountId(1), INITIAL_BALANCE),
        WitnessTestAccount::new_empty(AccountId(2)),
        WitnessTestAccount::new(AccountId(3), INITIAL_BALANCE),
    ];
    let (account_from, account_to, incorrect_account_to) =
        (&accounts[0], &accounts[1], &accounts[2]);
    let transfer_op = TransferOp {
        tx: account_from
            .zksync_account
            .sign_transfer(
                TOKEN_ID,
                "",
                BigUint::from(TOKEN_AMOUNT),
                BigUint::from(FEE_AMOUNT),
                &incorrect_account_to.account.address,
                None,
                true,
                Default::default(),
            )
            .0,
        from: account_from.id,
        to: account_to.id,
    };

    let input = SigDataInput::from_transfer_op(&transfer_op).expect("SigDataInput creation failed");

    incorrect_op_test_scenario::<TransferWitness<Bn256>, _, _>(
        &accounts,
        transfer_op,
        input,
        ERR_MSG,
        || {
            vec![CollectedFee {
                token: TOKEN_ID,
                amount: FEE_AMOUNT.into(),
            }]
        },
        |_| {},
    );
}

/// Checks that executing a transfer operation with incorrect
/// data (insufficient funds) results in an error.
#[test]
#[ignore]
fn test_incorrect_transfer_amount() {
    const TOKEN_ID: TokenId = TokenId(0);
    // Balance check should fail.
    // "balance-fee bits" is message for subtraction check in circuit.
    // For details see `circuit.rs`.
    const ERR_MSG: &str = "balance-fee bits";

    // Test vector of (initial_balance, transfer_amount, fee_amount).
    let test_vector = vec![
        (10u64, 15u64, 0u64), // Transfer too big
        (10, 7, 4),           // Fee too big
        (0, 1, 1),            // Transfer from 0 balance
    ];

    for (initial_balance, transfer_amount, fee_amount) in test_vector {
        // Input data: account does not have enough funds.
        let accounts = vec![
            WitnessTestAccount::new(AccountId(1), initial_balance),
            WitnessTestAccount::new_empty(AccountId(2)),
        ];
        let (account_from, account_to) = (&accounts[0], &accounts[1]);
        let transfer_op = TransferOp {
            tx: account_from
                .zksync_account
                .sign_transfer(
                    TOKEN_ID,
                    "",
                    BigUint::from(transfer_amount),
                    BigUint::from(fee_amount),
                    &account_to.account.address,
                    None,
                    true,
                    Default::default(),
                )
                .0,
            from: account_from.id,
            to: account_to.id,
        };

        let input =
            SigDataInput::from_transfer_op(&transfer_op).expect("SigDataInput creation failed");

        incorrect_op_test_scenario::<TransferWitness<Bn256>, _, _>(
            &accounts,
            transfer_op,
            input,
            ERR_MSG,
            || {
                vec![CollectedFee {
                    token: TOKEN_ID,
                    amount: fee_amount.into(),
                }]
            },
            |_| {},
        );
    }
}

/// Checks that even if there are two accounts with the same keys in the state,
/// one account cannot authorize the transfer from its duplicate.
#[test]
#[ignore]
fn test_transfer_replay() {
    const TOKEN_ID: TokenId = TokenId(0);
    const INITIAL_BALANCE: u64 = 10;
    const TOKEN_AMOUNT: u64 = 7;
    const FEE_AMOUNT: u64 = 3;

    // Operation is not valid, since the balance is already transferred from account
    // with the same private key.
    const ERR_MSG: &str = "op_valid is true/enforce equal to one";

    let account_base = WitnessTestAccount::new(AccountId(1), INITIAL_BALANCE);
    // Create a copy of the base account with the same keys.
    let mut account_copy = WitnessTestAccount::new_empty(AccountId(2));
    account_copy.account = account_base.account.clone();

    // Input data
    let accounts = vec![
        account_base,
        account_copy,
        WitnessTestAccount::new_empty(AccountId(3)),
    ];

    let (account_from, account_copy, account_to) = (&accounts[0], &accounts[1], &accounts[2]);

    // Create the transfer_op, and set the `from` ID to the duplicate account ID.
    // Despite that both account and duplicate account have the same keys, transfer
    // operation contains the account ID, and transaction should fail.
    let transfer_op = TransferOp {
        tx: account_from
            .zksync_account
            .sign_transfer(
                TOKEN_ID,
                "",
                BigUint::from(TOKEN_AMOUNT),
                BigUint::from(FEE_AMOUNT),
                &account_to.account.address,
                None,
                true,
                Default::default(),
            )
            .0,
        from: account_copy.id,
        to: account_to.id,
    };

    let input = SigDataInput::from_transfer_op(&transfer_op).expect("SigDataInput creation failed");

    incorrect_op_test_scenario::<TransferWitness<Bn256>, _, _>(
        &accounts,
        transfer_op,
        input,
        ERR_MSG,
        || {
            vec![CollectedFee {
                token: TOKEN_ID,
                amount: FEE_AMOUNT.into(),
            }]
        },
        |_| {},
    );
}

/// Basic check for execution of `Transfer` operation in circuit with incorrect timestamps.
#[test]
#[ignore]
fn test_incorrect_transfer_timestamp() {
    // Test vector of (initial_balance, transfer_amount, fee_amount, time_range).
    let test_vector = vec![
        (10u64, 7u64, 3u64, TimeRange::new(0, 0)),
        (10u64, 7u64, 3u64, TimeRange::new(0, BLOCK_TIMESTAMP - 1)),
        (
            10u64,
            7u64,
            3u64,
            TimeRange::new(BLOCK_TIMESTAMP + 1, u64::max_value()),
        ),
    ];

    for (initial_balance, transfer_amount, fee_amount, time_range) in test_vector {
        // Input data.
        let accounts = vec![
            WitnessTestAccount::new(AccountId(1), initial_balance),
            WitnessTestAccount::new_empty(AccountId(2)),
        ];
        let (account_from, account_to) = (&accounts[0], &accounts[1]);
        let transfer_op = TransferOp {
            tx: account_from
                .zksync_account
                .sign_transfer(
                    TokenId(0),
                    "",
                    BigUint::from(transfer_amount),
                    BigUint::from(fee_amount),
                    &account_to.account.address,
                    None,
                    true,
                    time_range,
                )
                .0,
            from: account_from.id,
            to: account_to.id,
        };

        // Additional data required for performing the operation.
        let input =
            SigDataInput::from_transfer_op(&transfer_op).expect("SigDataInput creation failed");

        // Operation is not valid, since transaction timestamp is invalid.
        const ERR_MSG: &str = "op_valid is true/enforce equal to one";

        incorrect_op_test_scenario::<TransferWitness<Bn256>, _, _>(
            &accounts,
            transfer_op,
            input,
            ERR_MSG,
            || {
                vec![CollectedFee {
                    token: TokenId(0),
                    amount: fee_amount.into(),
                }]
            },
            |_| {},
        );
    }
}

/// Basic check for execution of `Transfer` operation in circuit with nft token id as a token to process.
#[test]
#[ignore]
fn test_transfer_with_nft_token_id_as_a_token_to_process() {
    // Input data.
    let accounts = vec![
        WitnessTestAccount::new_empty(AccountId(1)),
        WitnessTestAccount::new_empty(AccountId(2)),
    ];
    let (account_from, account_to) = (&accounts[0], &accounts[1]);
    let mut tx = Transfer::new(
        AccountId(1),
        account_from.zksync_account.address,
        account_to.account.address,
        NFT_TOKEN_ID,
        BigUint::from(0u32),
        BigUint::from(0u32),
        Nonce(0),
        Default::default(),
        None,
    );
    tx.signature =
        TxSignature::sign_musig(&account_from.zksync_account.private_key, &tx.get_bytes());
    let transfer_op = TransferOp {
        tx,
        from: account_from.id,
        to: account_to.id,
    };

    // Additional data required for performing the operation.
    let input = SigDataInput::from_transfer_op(&transfer_op).expect("SigDataInput creation failed");

    const ERR_MSG: &str = "chunk number 1/execute_op/op_valid is true/enforce equal to one";

    incorrect_op_test_scenario::<TransferWitness<Bn256>, _, _>(
        &accounts,
        transfer_op,
        input,
        ERR_MSG,
        || {
            vec![CollectedFee {
                token: NFT_TOKEN_ID,
                amount: BigUint::from(0u32),
            }]
        },
        |_| {},
    );
}

/// Basic check for execution of `Transfer` operation in circuit with nft storage account id.
#[test]
#[ignore]
fn test_transfer_with_nft_storage_account_id() {
    // Input data.
    let accounts = vec![
        WitnessTestAccount::new_empty(AccountId(1)),
        WitnessTestAccount::new_empty(NFT_STORAGE_ACCOUNT_ID),
    ];
    let (account_from, account_to) = (&accounts[0], &accounts[1]);
    let transfer_op = TransferOp {
        tx: account_from
            .zksync_account
            .sign_transfer(
                TokenId(0),
                "",
                BigUint::from(0u32),
                BigUint::from(0u32),
                &account_to.account.address,
                None,
                true,
                Default::default(),
            )
            .0,
        from: account_from.id,
        to: account_to.id,
    };

    // Additional data required for performing the operation.
    let input = SigDataInput::from_transfer_op(&transfer_op).expect("SigDataInput creation failed");

    const ERR_MSG: &str = "chunk number 1/execute_op/op_valid is true/enforce equal to one";

    incorrect_op_test_scenario::<TransferWitness<Bn256>, _, _>(
        &accounts,
        transfer_op,
        input,
        ERR_MSG,
        || {
            vec![CollectedFee {
                token: TokenId(0),
                amount: BigUint::from(0u32),
            }]
        },
        |_| {},
    );
}
