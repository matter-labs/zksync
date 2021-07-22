use std::str::FromStr;
// External uses
// Workspace uses
use zksync_basic_types::Address;
use zksync_utils::format_units;
// Local uses
use crate::{tx::*, AccountId, Nonce, Token, TokenId, Transfer, Withdraw, ZkSyncTx};

fn get_transfer() -> Transfer {
    Transfer::new(
        AccountId(4242),
        Address::from_str("2e46cd9538248826ede540012c0e8d13f223d587").unwrap(),
        Address::random(),
        TokenId(0),
        500u32.into(),
        0u32.into(),
        Nonce(0),
        Default::default(),
        None,
    )
}

fn get_withdraw() -> Withdraw {
    Withdraw::new(
        AccountId(33),
        Address::from_str("8971d4b0ec2bc8324238c25f2516e9d823b7077b").unwrap(),
        Address::random(),
        TokenId(0),
        100u32.into(),
        10u32.into(),
        Nonce(12),
        Default::default(),
        None,
    )
}

fn get_change_pub_key() -> ChangePubKey {
    ChangePubKey::new(
        AccountId(123),
        Address::from_str("b9154aec27863a01d085d257f238f755a728f4e5").unwrap(),
        Default::default(),
        TokenId(0),
        Default::default(),
        Nonce(13),
        Default::default(),
        None,
        None,
    )
}

/// Checks that we can't create signature data from the empty batch.
#[test]
fn test_empty_batch() {
    assert!(EthBatchSignData::new(Vec::new(), Vec::new()).is_err());
}

/// Checks the correctness of the message `EthBatchSignData::new()` returns.
#[test]
fn test_batch_message() {
    let token = Token::new(TokenId(0), Default::default(), "ETH", 18);
    let transfer = get_transfer();
    let withdraw = get_withdraw();
    let change_pub_key = get_change_pub_key();
    let txs = vec![
        ZkSyncTx::from(transfer.clone()),
        ZkSyncTx::from(withdraw.clone()),
        ZkSyncTx::from(change_pub_key.clone()),
    ];

    let expected = format!(
        "From: 0x2e46cd9538248826ede540012c0e8d13f223d587\n\
        Transfer {amount1} {token} to: {to1:?}\n\
        Nonce: 0\n\
        \n\
        From: 0x8971d4b0ec2bc8324238c25f2516e9d823b7077b\n\
        Withdraw {amount2} {token} to: {to2:?}\n\
        Fee: {fee} {token}\n\
        Nonce: 12\n\
        \n\
        From: 0xb9154aec27863a01d085d257f238f755a728f4e5\n\
        Set signing key: {pub_key_hash}\n\
        Nonce: 13",
        amount1 = format_units(transfer.amount, 18),
        amount2 = format_units(withdraw.amount, 18),
        token = "ETH",
        to1 = transfer.to,
        to2 = withdraw.to,
        fee = format_units(withdraw.fee, 18),
        pub_key_hash = hex::encode(change_pub_key.new_pk_hash.data).to_ascii_lowercase()
    );

    let txs = txs
        .into_iter()
        .zip(std::iter::repeat(token.clone()))
        .map(|(tx, token)| (tx.clone(), token, tx.account()))
        .collect::<Vec<_>>();
    // Shouldn't fail.
    let batch_sign_data = EthBatchSignData::new(txs, Vec::new()).unwrap();
    assert_eq!(batch_sign_data.message, expected.into_bytes());

    // Batch from a single wallet, send withdraw without fee, cover the fee with phantom transfer.
    let mut withdraw = get_withdraw();
    let mut transfer = get_transfer();
    // Same sender.
    transfer.from = withdraw.from;
    // "Transfer..." line will be omitted.
    transfer.amount = 0u32.into();
    transfer.fee = withdraw.fee;
    // No fee for withdraw.
    withdraw.fee = 0u32.into();
    let txs = vec![
        ZkSyncTx::from(transfer.clone()),
        ZkSyncTx::from(withdraw.clone()),
    ];
    let expected = format!(
        "Fee: {fee} {token}\n\
        Withdraw {amount} {token} to: {to:?}\n\
        Nonce: 0",
        fee = format_units(transfer.fee, 18),
        token = "ETH",
        amount = format_units(withdraw.amount, 18),
        to = withdraw.to
    );
    let txs = txs
        .into_iter()
        .zip(std::iter::repeat(token))
        .map(|(tx, token)| (tx.clone(), token, tx.account()))
        .collect::<Vec<_>>();

    let message = EthBatchSignData::get_batch_sign_message(txs);
    assert_eq!(message, expected.into_bytes());
}
