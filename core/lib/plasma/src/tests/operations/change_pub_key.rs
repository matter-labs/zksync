use crate::tests::{AccountState::*, PlasmaTestBuilder};
use models::account::{AccountUpdate, PubKeyHash};
use models::tx::ChangePubKey;
use zksync_crypto::{params, rand::Rng};

#[test]
fn success() {
    let mut tb = PlasmaTestBuilder::new();
    let (account_id, account, _sk) = tb.add_account(Locked);
    let old_pub_key_hash = account.pub_key_hash.clone();
    let pubkey_bytes: [u8; params::FR_ADDRESS_LEN] = tb.rng.gen();
    let new_pub_key_hash = PubKeyHash::from_bytes(&pubkey_bytes).unwrap();

    let change_pub_key = ChangePubKey {
        account_id,
        account: account.address,
        new_pk_hash: new_pub_key_hash.clone(),
        nonce: account.nonce,
        eth_signature: None,
    };

    tb.test_tx_success(
        change_pub_key.into(),
        &[(
            account_id,
            AccountUpdate::ChangePubKeyHash {
                old_nonce: account.nonce,
                new_nonce: account.nonce + 1,
                old_pub_key_hash,
                new_pub_key_hash,
            },
        )],
    )
}

#[test]
fn nonce_mismatch() {
    let mut tb = PlasmaTestBuilder::new();
    let (account_id, account, _sk) = tb.add_account(Locked);
    let pubkey_bytes: [u8; params::FR_ADDRESS_LEN] = tb.rng.gen();
    let new_pub_key_hash = PubKeyHash::from_bytes(&pubkey_bytes).unwrap();

    let change_pub_key = ChangePubKey {
        account_id,
        account: account.address,
        new_pk_hash: new_pub_key_hash,
        nonce: account.nonce + 1,
        eth_signature: None,
    };

    tb.test_tx_fail(change_pub_key.into(), "Nonce mismatch");
}

#[test]
fn invalid_account_id() {
    let mut tb = PlasmaTestBuilder::new();
    let (account_id, account, _sk) = tb.add_account(Locked);
    let pubkey_bytes: [u8; params::FR_ADDRESS_LEN] = tb.rng.gen();
    let new_pub_key_hash = PubKeyHash::from_bytes(&pubkey_bytes).unwrap();

    let change_pub_key = ChangePubKey {
        account_id: account_id + 1,
        account: account.address,
        new_pk_hash: new_pub_key_hash,
        nonce: account.nonce,
        eth_signature: None,
    };

    tb.test_tx_fail(
        change_pub_key.into(),
        "ChangePubKey account id is incorrect",
    );
}
