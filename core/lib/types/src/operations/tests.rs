use num::BigUint;
use std::str::FromStr;

use zksync_basic_types::Address;

use super::*;
use crate::{
    account::PubKeyHash,
    priority_ops::{Deposit, FullExit},
    tx::{ChangePubKey, Close, ForcedExit, PackedEthSignature, Transfer, TxSignature, Withdraw},
};

// Public data parameters, using them we can restore `ZkSyncOp`.
const NOOP_PUBLIC_DATA: &str = "000000000000000000";
const DEPOSIT_PUBLIC_DATA: &str = "010000002a002a0000000000000000000000000000002a21abaed8712072e918632259780e587698ef58da0000000000000000000000";
const TRANSFER_TO_NEW_PUBLIC_DATA: &str = "0200000001002a000000054021abaed8712072e918632259780e587698ef58da00000002054000000000000000000000000000000000";
const WITHDRAW_PUBLIC_DATA: &str = "030000002a002a0000000000000000000000000000002a054021abaed8712072e918632259780e587698ef58da000000000000000000";
const CLOSE_PUBLIC_DATA: &str = "040000002a00000000";
const TRANSFER_PUBLIC_DATA: &str = "0500000001002a0000000200000005400540";
const FULL_EXIT_PUBLIC_DATA: &str = "060000002a2a0a81e257a2f5d6ed4f07b81dbda09f107bd026002a000000000000000000000000000000000000000000000000000000";
const CHANGE_PUBKEY_PUBLIC_DATA: &str = "070000002a3cfb9a39096d9e02b24187355f628f9a6331511b2a0a81e257a2f5d6ed4f07b81dbda09f107bd0260000002a002a054000";
const FORCED_EXIT_PUBLIC_DATA: &str = "080000002a0000002a002a0000000000000000000000000000000005402a0a81e257a2f5d6ed4f07b81dbda09f107bd0260000000000";

#[test]
fn test_public_data_convetions_noop() {
    let expected_op = NoopOp {};

    assert_eq!(hex::encode(expected_op.get_public_data()), NOOP_PUBLIC_DATA);
}

#[test]
fn test_public_data_convetions_deposit() {
    let expected_op = {
        let priority_op = Deposit {
            from: Address::from_str("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026").unwrap(),
            to: Address::from_str("21abaed8712072e918632259780e587698ef58da").unwrap(),
            token: 42,
            amount: BigUint::from(42u32),
        };
        let account_id = 42u32;

        DepositOp {
            priority_op,
            account_id,
        }
    };

    assert_eq!(
        hex::encode(expected_op.get_public_data()),
        DEPOSIT_PUBLIC_DATA
    );
}

#[test]
fn test_public_data_convetions_transfer() {
    let (expected_transfer, expected_transfer_to_new) = {
        let tx = Transfer::new(
            42,
            Address::from_str("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026").unwrap(),
            Address::from_str("21abaed8712072e918632259780e587698ef58da").unwrap(),
            42,
            BigUint::from(42u32),
            BigUint::from(42u32),
            42,
            None,
        );
        let (from, to) = (1u32, 2u32);

        (
            TransferOp {
                tx: tx.clone(),
                from,
                to,
            },
            TransferToNewOp { tx, from, to },
        )
    };

    assert_eq!(
        hex::encode(expected_transfer.get_public_data()),
        TRANSFER_PUBLIC_DATA
    );
    assert_eq!(
        hex::encode(expected_transfer_to_new.get_public_data()),
        TRANSFER_TO_NEW_PUBLIC_DATA
    );
}

#[test]
fn test_public_data_convetions_withdraw() {
    let expected_op = {
        let tx = Withdraw::new(
            42,
            Address::from_str("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026").unwrap(),
            Address::from_str("21abaed8712072e918632259780e587698ef58da").unwrap(),
            42,
            BigUint::from(42u32),
            BigUint::from(42u32),
            42,
            None,
        );
        let account_id = 42u32;

        WithdrawOp { tx, account_id }
    };

    assert_eq!(
        hex::encode(expected_op.get_public_data()),
        WITHDRAW_PUBLIC_DATA
    );
}

#[test]
fn test_public_data_convetions_close() {
    let expected_op = {
        let tx = Close {
            account: Address::from_str("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026").unwrap(),
            nonce: 42,
            signature: TxSignature::default(),
        };
        let account_id = 42;

        CloseOp { tx, account_id }
    };

    assert_eq!(
        hex::encode(expected_op.get_public_data()),
        CLOSE_PUBLIC_DATA
    );
}

#[test]
fn test_public_data_convetions_full_exit() {
    let expected_op = {
        let priority_op = FullExit {
            eth_address: Address::from_str("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026").unwrap(),
            account_id: 42,
            token: 42,
        };

        FullExitOp {
            priority_op,
            withdraw_amount: None,
        }
    };

    assert_eq!(
        hex::encode(expected_op.get_public_data()),
        FULL_EXIT_PUBLIC_DATA
    );
}

#[test]
fn test_public_data_convetions_change_pubkey() {
    let expected_op = {
        let tx = ChangePubKey::new(
                42,
                Address::from_str("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026").unwrap(),
                PubKeyHash::from_hex("sync:3cfb9a39096d9e02b24187355f628f9a6331511b").unwrap(),
                42,
                BigUint::from(42u32),
                42,
                None,
                Some(PackedEthSignature::deserialize_packed(
                    &hex::decode("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026dbda09f107bd026f5d6ed4f02a0a81e257a2f5d6ed4f07b81dbda09f107bd026dbda09f107bd026f5d6ed4f0d4").unwrap(),
                ).unwrap()),
            );
        let account_id = 42u32;

        ChangePubKeyOp { tx, account_id }
    };

    assert_eq!(
        hex::encode(expected_op.get_public_data()),
        CHANGE_PUBKEY_PUBLIC_DATA
    );
}

#[test]
fn test_public_data_convetions_forced_exit() {
    let expected_op = {
        let tx = ForcedExit::new(
            42,
            Address::from_str("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026").unwrap(),
            42,
            BigUint::from(42u32),
            42,
            None,
        );
        let target_account_id = 42u32;

        ForcedExitOp {
            tx,
            target_account_id,
            withdraw_amount: None,
        }
    };

    assert_eq!(
        hex::encode(expected_op.get_public_data()),
        FORCED_EXIT_PUBLIC_DATA
    );
}

#[test]
fn test_withdrawal_data() {
    let (withdraw, forced_exit, full_exit) = (
        WithdrawOp::from_public_data(&hex::decode(WITHDRAW_PUBLIC_DATA).unwrap()).unwrap(),
        ForcedExitOp::from_public_data(&hex::decode(FORCED_EXIT_PUBLIC_DATA).unwrap()).unwrap(),
        FullExitOp::from_public_data(&hex::decode(FULL_EXIT_PUBLIC_DATA).unwrap()).unwrap(),
    );

    assert_eq!(
        hex::encode(withdraw.get_withdrawal_data()),
        "0121abaed8712072e918632259780e587698ef58da002a0000000000000000000000000000002a"
    );
    assert_eq!(
        hex::encode(forced_exit.get_withdrawal_data()),
        "012a0a81e257a2f5d6ed4f07b81dbda09f107bd026002a00000000000000000000000000000000"
    );
    assert_eq!(
        hex::encode(full_exit.get_withdrawal_data()),
        "002a0a81e257a2f5d6ed4f07b81dbda09f107bd026002a00000000000000000000000000000000"
    );
}

#[test]
fn test_eth_witness() {
    let mut change_pubkey =
        ChangePubKeyOp::from_public_data(&hex::decode(CHANGE_PUBKEY_PUBLIC_DATA).unwrap()).unwrap();

    change_pubkey.tx.eth_signature = PackedEthSignature::deserialize_packed(
            &hex::decode("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026dbda09f107bd026f5d6ed4f02a0a81e257a2f5d6ed4f07b81dbda09f107bd026dbda09f107bd026f5d6ed4f0d4").unwrap(),
        ).ok();

    assert_eq!(
            hex::encode(change_pubkey.get_eth_witness()),
            "2a0a81e257a2f5d6ed4f07b81dbda09f107bd026dbda09f107bd026f5d6ed4f02a0a81e257a2f5d6ed4f07b81dbda09f107bd026dbda09f107bd026f5d6ed4f0d4"
        );
}
