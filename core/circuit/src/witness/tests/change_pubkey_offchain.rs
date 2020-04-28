// External deps
use crypto_exports::franklin_crypto::bellman::pairing::bn256::Bn256;
// Workspace deps
use models::node::operations::ChangePubKeyOp;
// Local deps
use crate::witness::{
    change_pubkey_offchain::ChangePubkeyOffChainWitness,
    tests::test_utils::{generic_test_scenario, WitnessTestAccount},
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
