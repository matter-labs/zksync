use std::ops::Deref;
// External uses
use anyhow::Result;
use parity_crypto::publickey::{Generator, Random};
// Workspace uses
use zksync_basic_types::{Address, H256};
// Local uses
use super::packed_eth_signature::PackedEthSignature;
use crate::{tx::*, Transfer, Withdraw, ZkSyncTx};

fn get_packed_signature() -> PackedEthSignature {
    let keypair = Random.generate();
    let private_key = keypair.secret();
    PackedEthSignature::sign(private_key.deref(), b"test").unwrap()
}

fn get_eth_signature() -> TxEthSignature {
    TxEthSignature::EthereumSignature(get_packed_signature())
}

fn get_batch() -> Vec<ZkSyncTx> {
    let transfer = Transfer::new(
        4242,
        Address::random(),
        Address::random(),
        0,
        500u32.into(),
        20u32.into(),
        11,
        None,
    );

    let withdraw = Withdraw::new(
        33,
        Address::random(),
        Address::random(),
        0,
        100u32.into(),
        10u32.into(),
        12,
        None,
    );

    let change_pubkey = ChangePubKey::new(
        123,
        Address::random(),
        Default::default(),
        0,
        Default::default(),
        13,
        None,
        Some(get_packed_signature()),
    );

    vec![
        ZkSyncTx::Transfer(Box::new(transfer)),
        ZkSyncTx::Withdraw(Box::new(withdraw)),
        ZkSyncTx::ChangePubKey(Box::new(change_pubkey)),
    ]
}

/// Checks that we can't create signature data from the empty batch.
#[test]
fn test_empty_batch() {
    let txs = vec![];
    let signature = get_eth_signature();

    assert!(BatchSignData::new(&txs, signature).is_err());
}

/// Checks that we can't create batch signature data from the batch with multiple
/// `ChangePubKey` transactions.
#[test]
fn multiple_change_pub_key_in_batch() {
    let mut txs = get_batch();
    txs.push(txs.last().unwrap().clone());
    let signature = get_eth_signature();

    // Should return error.
    assert!(BatchSignData::new(&txs, signature).is_err());
}

/// Checks the correctness of the message `BatchSignData::new()` returns.
#[test]
fn test_batch_message() -> Result<()> {
    let mut txs = get_batch();
    let signature = get_eth_signature();

    // For the initial batch we expect prefixed hash of transactions data.
    let mut batch_hash = Vec::new();
    for tx in &txs {
        batch_hash.extend(tx.get_bytes().iter());
    }
    let batch_hash = tiny_keccak::keccak256(&batch_hash).to_vec();
    // Set the batch hash for ChangePubKey and get the message.
    let change_pub_key = match txs.last_mut().unwrap() {
        ZkSyncTx::ChangePubKey(tx) => tx,
        _ => panic!("ChangePubKey is supposed to be the last element in Vec of test transactions"),
    };
    change_pub_key.as_mut().batch_hash = H256::from_slice(batch_hash.as_slice());
    let change_pub_key_message = change_pub_key.as_ref().get_eth_signed_data()?;
    assert!(change_pub_key_message.ends_with(batch_hash.as_slice()));
    // Shouldn't fail.
    let batch_sign_data = BatchSignData::new(&txs, signature.clone())?;

    assert_eq!(batch_sign_data.0.message, change_pub_key_message);

    // Now remove `ChangePubKey` from the batch and expect the hash of bytes without the prefix.
    txs.pop();
    let mut batch_hash = Vec::new();
    for tx in &txs {
        batch_hash.extend(tx.get_bytes().iter());
    }
    let batch_hash = tiny_keccak::keccak256(&batch_hash);
    // Still shouldn't fail.
    let batch_sign_data = BatchSignData::new(&txs, signature)?;

    assert_eq!(batch_sign_data.0.message, batch_hash);
    Ok(())
}
