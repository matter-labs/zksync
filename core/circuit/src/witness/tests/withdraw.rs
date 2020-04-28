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
    // Input data.
    let accounts = vec![WitnessTestAccount::new(1, 10)];
    let account = &accounts[0];
    let withdraw_op = WithdrawOp {
        tx: account
            .zksync_account
            .sign_withdraw(
                0,
                "",
                BigDecimal::from(7),
                BigDecimal::from(3),
                &Address::zero(),
                None,
                true,
            )
            .0,
        account_id: account.id,
    };

    // Additional data required for performing the operation.
    let input = SigDataInput::from_withdraw_op(&withdraw_op).expect("SigDataInput creation failed");

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
