// External deps
use bigdecimal::BigDecimal;
use crypto_exports::franklin_crypto::bellman::pairing::bn256::Bn256;
// Workspace deps
use models::node::operations::TransferOp;
// Local deps
use crate::witness::{
    tests::test_utils::{generic_test_scenario, WitnessTestAccount},
    transfer::TransferWitness,
    utils::SigDataInput,
};

/// Basic check for execution of `Transfer` operation in circuit.
/// Here we create two accounts and perform a transfer between them.
#[test]
#[ignore]
fn test_transfer_success() {
    // Test vector of (initial_balance, transfer_amount, fee_amount).
    let test_vector = vec![
        (10, 7, 3),                // Basic transfer
        (0, 0, 0),                 // Zero transfer
        (std::u64::MAX, 1, 1),     // Small transfer from rich account,
        (std::u64::MAX, 10000, 1), // Big transfer from rich account (too big values can't be used, since they're not packable),
        (std::u64::MAX, 1, 10000), // Very big fee
    ];

    for (initial_balance, transfer_amount, fee_amount) in test_vector {
        // Input data.
        let accounts = vec![
            WitnessTestAccount::new(1, initial_balance),
            WitnessTestAccount::new_empty(2),
        ];
        let (account_from, account_to) = (&accounts[0], &accounts[1]);
        let transfer_op = TransferOp {
            tx: account_from
                .zksync_account
                .sign_transfer(
                    0,
                    "",
                    BigDecimal::from(transfer_amount),
                    BigDecimal::from(fee_amount),
                    &account_to.account.address,
                    None,
                    true,
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
                let (fee, _) = plasma_state
                    .apply_transfer_op(&op)
                    .expect("transfer should be success");
                vec![fee]
            },
        );
    }
}

/// Check for execution of `Transfer` operation with recipient same as sender in circuit.
/// Here we create one accounts and perform a transfer to self.
#[test]
#[ignore]
fn test_transfer_to_self() {
    // Input data.
    let accounts = vec![WitnessTestAccount::new(1, 10)];
    let account = &accounts[0];
    let transfer_op = TransferOp {
        tx: account
            .zksync_account
            .sign_transfer(
                0,
                "",
                BigDecimal::from(7),
                BigDecimal::from(3),
                &account.account.address,
                None,
                true,
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
            let (fee, _) = plasma_state
                .apply_transfer_op(&op)
                .expect("transfer should be success");
            vec![fee]
        },
    );
}
