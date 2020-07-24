// External deps
use crypto_exports::franklin_crypto::bellman::pairing::bn256::Bn256;
use num::BigUint;
// Workspace deps
// Local deps
use crate::witness::{
    tests::test_utils::{generic_test_scenario, WitnessTestAccount},
    utils::SigDataInput,
    TransferFromWitness,
};
use models::node::{TransferFrom, TransferFromOp};

/// Basic check for execution of `Transfer` operation in circuit.
/// Here we create two accounts and perform a transfer between them.
#[test]
#[ignore]
fn test_transfer_from_success() {
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
            WitnessTestAccount::new(1, initial_balance),
            WitnessTestAccount::new_empty(2),
        ];
        let (account_from, account_to) = (&accounts[0], &accounts[1]);
        let tx = TransferFrom::new_signed(
            account_to.id,
            account_from.account.address,
            account_to.account.address,
            0,
            BigUint::from(transfer_amount),
            BigUint::from(fee_amount),
            account_from.account.nonce,
            0,
            u64::from(u32::max_value()),
            &account_from.zksync_account.private_key,
            &account_to.zksync_account.private_key,
        )
        .expect("failed to sign transfer from");
        let transfer_op = TransferFromOp {
            tx,
            from: account_from.id,
            to: account_to.id,
        };

        // Additional data required for performing the operation.
        let input = SigDataInput::from_transfer_from_op(&transfer_op)
            .expect("SigDataInput creation failed");

        generic_test_scenario::<TransferFromWitness<Bn256>, _>(
            &accounts,
            transfer_op,
            input,
            |plasma_state, op| {
                let (fee, _) = plasma_state
                    .apply_transfer_from_op(&op)
                    .expect("transfer should be success");
                vec![fee]
            },
        );
    }
}
