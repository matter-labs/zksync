use crate::tests::{AccountState::*, PlasmaTestBuilder};
use zksync_types::account::{AccountUpdate, PubKeyHash};
use zksync_types::tx::ChangePubKey;

/// Check ChangePubKey operation on new account
#[test]
fn success() {
    let mut tb = PlasmaTestBuilder::new();
    let token_id = 1;
    let balance = 10u32;
    let (account_id, account, sk) = tb.add_account(Locked);
    tb.set_balance(account_id, token_id, balance);
    let old_pub_key_hash = account.pub_key_hash.clone();
    let new_pub_key_hash = PubKeyHash::from_privkey(&sk);

    let change_pub_key = ChangePubKey::new_signed(
        account_id,
        account.address,
        new_pub_key_hash.clone(),
        token_id,
        balance.into(),
        account.nonce,
        None,
        &sk,
    )
    .expect("Failed to sign ChangePubkey");

    tb.test_tx_success(
        change_pub_key.into(),
        &[
            (
                account_id,
                AccountUpdate::ChangePubKeyHash {
                    old_nonce: account.nonce,
                    new_nonce: account.nonce + 1,
                    old_pub_key_hash,
                    new_pub_key_hash,
                },
            ),
            (
                account_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: account.nonce + 1,
                    new_nonce: account.nonce + 1,
                    balance_update: (token_id, balance.into(), 0u32.into()),
                },
            ),
        ],
    )
}

/// Check that ChangePubKey fails if nonce is invalid
#[test]
fn nonce_mismatch() {
    let mut tb = PlasmaTestBuilder::new();
    let (account_id, account, sk) = tb.add_account(Locked);
    let new_pub_key_hash = PubKeyHash::from_privkey(&sk);

    let change_pub_key = ChangePubKey::new_signed(
        account_id,
        account.address,
        new_pub_key_hash,
        0,
        0u32.into(),
        account.nonce + 1,
        None,
        &sk,
    )
    .expect("Failed to sign ChangePubkey");

    tb.test_tx_fail(change_pub_key.into(), "Nonce mismatch");
}

/// Check that ChangePubKey fails if account address
/// does not correspond to account_id
#[test]
fn invalid_account_id() {
    let mut tb = PlasmaTestBuilder::new();
    let (account_id, account, sk) = tb.add_account(Locked);
    let new_pub_key_hash = PubKeyHash::from_privkey(&sk);

    let change_pub_key = ChangePubKey::new_signed(
        account_id + 1,
        account.address,
        new_pub_key_hash,
        0,
        0u32.into(),
        account.nonce + 1,
        None,
        &sk,
    )
    .expect("Failed to sign ChangePubkey");

    tb.test_tx_fail(
        change_pub_key.into(),
        "ChangePubKey account id is incorrect",
    );
}
