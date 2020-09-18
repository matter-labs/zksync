// External deps
use crypto_exports::franklin_crypto::bellman::pairing::bn256::Bn256;
// Workspace deps
use models::node::operations::ChangePubKeyOp;
use plasma::state::CollectedFee;
// Local deps
use crate::witness::{
    change_pubkey_offchain::ChangePubkeyOffChainWitness,
    tests::test_utils::{generic_test_scenario, incorrect_op_test_scenario, WitnessTestAccount},
};

/// Basic check for execution of `ChangePubKeyOp` in circuit.
/// Here we generate an empty account and change its public key.
#[test]
#[ignore]
fn test_change_pubkey_offchain_success() {
    // Input data.
    let accounts = vec![WitnessTestAccount::new_empty(0xc1)];
    let account = &accounts[0];
    let change_pkhash_op = ChangePubKeyOp {
        tx: account
            .zksync_account
            .create_change_pubkey_tx(None, true, false),
        account_id: account.id,
    };

    generic_test_scenario::<ChangePubkeyOffChainWitness<Bn256>, _>(
        &accounts,
        change_pkhash_op,
        (),
        |plasma_state, op| {
            let fee = plasma_state
                .apply_change_pubkey_op(op)
                .expect("Operation failed")
                .0;

            vec![fee]
        },
    );
}

/// Checks that executing a change pubkey operation with incorrect
/// data (account `from` ID) results in an error.
#[test]
#[ignore]
#[should_panic(expected = "change pubkey address tx mismatch")]
fn test_incorrect_change_pubkey_account() {
    // Error message is not important, since we expect code to panic.
    const ERR_MSG: &str = "";

    // Input data: transaction is signed by an incorrect account (address of account
    // and ID of the `from` accounts differ).
    let incorrect_from_account = WitnessTestAccount::new_empty(3);

    let accounts = vec![WitnessTestAccount::new_empty(0xc1)];
    let account = &accounts[0];
    let change_pkhash_op = ChangePubKeyOp {
        tx: incorrect_from_account
            .zksync_account
            .create_change_pubkey_tx(None, true, false),
        account_id: account.id,
    };

    incorrect_op_test_scenario::<ChangePubkeyOffChainWitness<Bn256>, _>(
        &accounts,
        change_pkhash_op,
        (),
        ERR_MSG,
        || {
            vec![CollectedFee {
                token: 0,
                amount: 0u32.into(),
            }]
        },
    );
}
