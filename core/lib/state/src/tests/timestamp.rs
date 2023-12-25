use chrono::Utc;
use num::BigUint;
use zksync_types::{
    account::PubKeyHash,
    tx::{ChangePubKey, TimeRange, Withdraw},
    SignedZkSyncTx, TokenId, ZkSyncTx,
};

use crate::tests::{AccountState::*, PlasmaTestBuilder};
/// Check that transaction fails if timestamp is invalid
#[test]
fn invalid_timestamp_valid_from() {
    let mut tb = PlasmaTestBuilder::new();
    let (account_id, account, sk) = tb.add_account(Locked);
    let new_pub_key_hash = PubKeyHash::from_privkey(&sk);

    let time_range = TimeRange {
        valid_from: tb.block_timestamp + 1,
        ..Default::default()
    };

    let change_pub_key = ChangePubKey::new_signed(
        account_id,
        account.address,
        new_pub_key_hash,
        TokenId(0),
        0u32.into(),
        account.nonce + 1,
        time_range,
        None,
        &sk,
        None,
    )
    .expect("Failed to sign ChangePubkey");

    tb.test_tx_fail(
        change_pub_key.into(),
        "The transaction can't be executed in the block because of an invalid timestamp",
    );
}

/// Check that transaction fails if timestamp is invalid
#[test]
fn invalid_timestamp_valid_until() {
    const CURRENT_TIMESTAMP: u64 = 100;

    let mut tb = PlasmaTestBuilder::new();
    let (account_id, account, sk) = tb.add_account(Locked);
    let new_pub_key_hash = PubKeyHash::from_privkey(&sk);

    tb.set_timestamp(CURRENT_TIMESTAMP);

    let time_range = TimeRange {
        valid_until: tb.block_timestamp - 1,
        ..Default::default()
    };

    let change_pub_key = ChangePubKey::new_signed(
        account_id,
        account.address,
        new_pub_key_hash,
        TokenId(0),
        0u32.into(),
        account.nonce + 1,
        time_range,
        None,
        &sk,
        None,
    )
    .expect("Failed to sign ChangePubkey");

    tb.test_tx_fail(
        change_pub_key.into(),
        "The transaction can't be executed in the block because of an invalid timestamp",
    );
}

/// Check that batch fails if timestamp is invalid
#[test]
fn batch_invalid_timestamp() {
    let token_id = TokenId(0);
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();
    let (account_id, account, sk) = tb.add_account(Unlocked);
    tb.set_balance(account_id, token_id, &amount + &fee);
    let new_pub_key_hash = PubKeyHash::from_privkey(&sk);

    let time_range = TimeRange {
        valid_from: tb.block_timestamp + 1,
        ..Default::default()
    };

    let change_pub_key = ChangePubKey::new_signed(
        account_id,
        account.address,
        new_pub_key_hash,
        TokenId(0),
        0u32.into(),
        account.nonce + 1,
        time_range,
        None,
        &sk,
        None,
    )
    .expect("Failed to sign ChangePubkey");

    let withdraw = Withdraw::new_signed(
        account_id,
        account.address,
        account.address,
        token_id,
        amount,
        fee,
        account.nonce,
        Default::default(),
        &sk,
    )
    .unwrap();

    let signed_zk_sync_tx1 = SignedZkSyncTx {
        tx: ZkSyncTx::ChangePubKey(Box::new(change_pub_key)),
        eth_sign_data: None,
        created_at: Utc::now(),
    };
    let signed_zk_sync_tx2 = SignedZkSyncTx {
        tx: ZkSyncTx::Withdraw(Box::new(withdraw)),
        eth_sign_data: None,
        created_at: Utc::now(),
    };
    tb.test_txs_batch_fail(
        &[signed_zk_sync_tx1.clone(), signed_zk_sync_tx2.clone()],
        "Batch execution failed, since tx #1 of batch failed with a reason: The transaction can't be executed in the block because of an invalid timestamp",
    );
    // Just in case: check that if tx is not first, it still will fail.
    tb.test_txs_batch_fail(
        &[signed_zk_sync_tx2, signed_zk_sync_tx1],
        "Batch execution failed, since tx #2 of batch failed with a reason: The transaction can't be executed in the block because of an invalid timestamp",
    );
}
