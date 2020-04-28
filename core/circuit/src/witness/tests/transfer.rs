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
    // Input data.
    let accounts = vec![
        WitnessTestAccount::new(1, 10),
        WitnessTestAccount::new_empty(2),
    ];
    let (account_from, account_to) = (&accounts[0], &accounts[1]);
    let transfer_op = TransferOp {
        tx: account_from
            .zksync_account
            .sign_transfer(
                0,
                "",
                BigDecimal::from(7),
                BigDecimal::from(3),
                &account_to.account.address,
                None,
                true,
            )
            .0,
        from: account_from.id,
        to: account_to.id,
    };

    // Additional data required for performing the operation.
    let input = SigDataInput::from_transfer_op(&transfer_op);

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
    let input = SigDataInput::from_transfer_op(&transfer_op);

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
