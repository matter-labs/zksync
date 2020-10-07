// External deps
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
// Workspace deps
use zksync_state::state::CollectedFee;
use zksync_state::{handler::TxHandler, state::ZkSyncState};
use zksync_types::{
    tx::{ChangePubKey, TxSignature},
    ChangePubKeyOp,
};
// Local deps
use crate::witness::{
    change_pubkey_offchain::ChangePubkeyOffChainWitness,
    tests::test_utils::{generic_test_scenario, incorrect_op_test_scenario, WitnessTestAccount},
    utils::SigDataInput,
};

const FEE_TOKEN: u16 = 0; // ETH

/// Basic check for execution of `ChangePubKeyOp` in circuit.
/// Here we generate an empty account and change its public key.
#[test]
#[ignore]
fn test_change_pubkey_offchain_success() {
    // Input data.
    let accounts = vec![WitnessTestAccount::new_empty(0xc1)];
    let account = &accounts[0];
    let change_pkhash_op = ChangePubKeyOp {
        tx: account.zksync_account.sign_change_pubkey_tx(
            None,
            true,
            FEE_TOKEN,
            Default::default(),
            false,
        ),
        account_id: account.id,
    };

    let input = SigDataInput::from_change_pubkey_op(&change_pkhash_op)
        .expect("SigDataInput creation failed");

    generic_test_scenario::<ChangePubkeyOffChainWitness<Bn256>, _>(
        &accounts,
        change_pkhash_op,
        input,
        |plasma_state, op| {
            let fee = <ZkSyncState as TxHandler<ChangePubKey>>::apply_op(plasma_state, op)
                .expect("Operation failed")
                .0
                .unwrap();

            vec![fee]
        },
    );
}

/// Same as `test_change_pubkey_offchain_success`, but uses a nonzero fee value.
#[test]
#[ignore]
fn test_change_pubkey_offchain_nonzero_fee() {
    // Input data.
    let fee = 150u64.into();
    let accounts = vec![WitnessTestAccount::new(0xc1, 500u64)];
    let account = &accounts[0];
    let change_pkhash_op = ChangePubKeyOp {
        tx: account
            .zksync_account
            .sign_change_pubkey_tx(None, true, FEE_TOKEN, fee, false),
        account_id: account.id,
    };

    let input = SigDataInput::from_change_pubkey_op(&change_pkhash_op)
        .expect("SigDataInput creation failed");

    generic_test_scenario::<ChangePubkeyOffChainWitness<Bn256>, _>(
        &accounts,
        change_pkhash_op,
        input,
        |plasma_state, op| {
            let fee = <ZkSyncState as TxHandler<ChangePubKey>>::apply_op(plasma_state, op)
                .expect("Operation failed")
                .0
                .unwrap();

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
        tx: incorrect_from_account.zksync_account.sign_change_pubkey_tx(
            None,
            true,
            FEE_TOKEN,
            Default::default(),
            false,
        ),
        account_id: account.id,
    };

    let input = SigDataInput::from_change_pubkey_op(&change_pkhash_op)
        .expect("SigDataInput creation failed");

    incorrect_op_test_scenario::<ChangePubkeyOffChainWitness<Bn256>, _>(
        &accounts,
        change_pkhash_op,
        input,
        ERR_MSG,
        || {
            vec![CollectedFee {
                token: 0,
                amount: 0u32.into(),
            }]
        },
    );
}

/// Checks that executing a change pubkey operation with incorrect
/// signature (account `from` ID) results in an error.
///
/// In this test we attempt the following attack scenario:
/// dishonest operator changes a fee in the transaction and signs it with a random key.
/// The pubkey hash inside of operation remains correct, so that there will be no rules
/// violation in contract. Circuit must detect that signature doesn't correspond to the
/// pubkey hash set inside of transaction.
#[test]
#[ignore]
fn test_incorrect_change_pubkey_signature() {
    // Error message is not important, since we expect code to panic.
    const ERR_MSG: &str = "chunk number 0/execute_op/op_valid is true/enforce equal to one";
    const ACCOUNT_ID: u32 = 0xc1;
    const HIJACK_FEE_AMOUNT: u32 = 100;

    // Input data: account with the same ID, but different key..
    let incorrect_account = WitnessTestAccount::new_empty(ACCOUNT_ID);

    let accounts = vec![WitnessTestAccount::new(0xc1, 1000)];
    let account = &accounts[0];

    // Create a signed message and replace its signature.
    let mut hijacked_tx = account.zksync_account.sign_change_pubkey_tx(
        None,
        true,
        FEE_TOKEN,
        Default::default(),
        false,
    );

    // Change fee.
    hijacked_tx.fee += HIJACK_FEE_AMOUNT;
    // And now sign this message again with a different key.
    hijacked_tx.signature = TxSignature::sign_musig(
        &incorrect_account.zksync_account.private_key,
        &hijacked_tx.get_bytes(),
    );

    let change_pkhash_op = ChangePubKeyOp {
        tx: hijacked_tx,
        account_id: account.id,
    };

    let input = SigDataInput::from_change_pubkey_op(&change_pkhash_op)
        .expect("SigDataInput creation failed");

    incorrect_op_test_scenario::<ChangePubkeyOffChainWitness<Bn256>, _>(
        &accounts,
        change_pkhash_op,
        input,
        ERR_MSG,
        || {
            vec![CollectedFee {
                token: 0,
                amount: HIJACK_FEE_AMOUNT.into(),
            }]
        },
    );
}
