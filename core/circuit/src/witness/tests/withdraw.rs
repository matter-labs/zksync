// External deps
use bigdecimal::BigDecimal;
use crypto_exports::franklin_crypto::bellman::pairing::bn256::Bn256;
// Workspace deps
use models::node::{operations::WithdrawOp, Address};
// Local deps
use crate::witness::{
    tests::test_utils::{generic_test_scenario, WitnessTestAccount},
    utils::SigDataInput,
    withdraw::WithdrawWitness,
};

#[test]
#[ignore]
fn test_withdraw() {
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
        let accounts = vec![WitnessTestAccount::new(1, initial_balance)];
        let account = &accounts[0];
        let withdraw_op = WithdrawOp {
            tx: account
                .zksync_account
                .sign_withdraw(
                    0,
                    "",
                    BigDecimal::from(transfer_amount),
                    BigDecimal::from(fee_amount),
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
                let (fee, _) = plasma_state
                    .apply_withdraw_op(&op)
                    .expect("transfer should be success");
                vec![fee]
            },
        );
    }
}
