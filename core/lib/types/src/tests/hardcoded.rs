//! Tests strictly validating the output for hardcoded input data
//!
//! These tests only check the code that should not change, and if it does change, then this may be a critical change.
//!  
//! If after changing some code, the tests stopped passing, then not only change the expected
//! answer for the tests but also be sure to notify the command about the changes introduced!

use num::BigUint;
use std::convert::TryFrom;
use std::str::FromStr;
use web3::types::Bytes;

use crate::{
    account::PubKeyHash,
    operations::{
        ChangePubKeyOp, DepositOp, ForcedExitOp, FullExitOp, NoopOp, TransferOp, TransferToNewOp,
        WithdrawOp,
    },
    priority_ops::{Deposit, FullExit},
    tx::{ChangePubKey, ForcedExit, PackedEthSignature, Transfer, Withdraw},
    Log, PriorityOp,
};
use lazy_static::lazy_static;
use zksync_basic_types::{Address, H256};

#[cfg(test)]
pub mod operations_test {
    use super::*;
    // Public data parameters, using them we can restore `ZkSyncOp`.
    const NOOP_PUBLIC_DATA: &str = "000000000000000000";
    const DEPOSIT_PUBLIC_DATA: &str = "010000002a002a0000000000000000000000000000002a21abaed8712072e918632259780e587698ef58da0000000000000000000000";
    const TRANSFER_TO_NEW_PUBLIC_DATA: &str = "0200000001002a000000054021abaed8712072e918632259780e587698ef58da00000002054000000000000000000000000000000000";
    const WITHDRAW_PUBLIC_DATA: &str = "030000002a002a0000000000000000000000000000002a054021abaed8712072e918632259780e587698ef58da000000000000000000";
    const TRANSFER_PUBLIC_DATA: &str = "0500000001002a0000000200000005400540";
    const FULL_EXIT_PUBLIC_DATA: &str = "060000002a2a0a81e257a2f5d6ed4f07b81dbda09f107bd026002a000000000000000000000000000000000000000000000000000000";
    const CHANGE_PUBKEY_PUBLIC_DATA: &str = "070000002a3cfb9a39096d9e02b24187355f628f9a6331511b2a0a81e257a2f5d6ed4f07b81dbda09f107bd0260000002a002a054000";
    const FORCED_EXIT_PUBLIC_DATA: &str = "080000002a0000002a002a0000000000000000000000000000000005402a0a81e257a2f5d6ed4f07b81dbda09f107bd0260000000000";

    #[test]
    fn test_public_data_conversions_noop() {
        let expected_op = NoopOp {};

        assert_eq!(hex::encode(expected_op.get_public_data()), NOOP_PUBLIC_DATA);
    }

    #[test]
    fn test_public_data_conversions_deposit() {
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
    fn test_public_data_conversions_transfer() {
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
    fn test_public_data_conversions_withdraw() {
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
    fn test_public_data_conversions_full_exit() {
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
    fn test_public_data_conversions_change_pubkey() {
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
    fn test_public_data_conversions_forced_exit() {
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
        // TODO: Change pre-defined input / output after merging breaking to dev (ZKS-131).

        let mut change_pubkey =
            ChangePubKeyOp::from_public_data(&hex::decode(CHANGE_PUBKEY_PUBLIC_DATA).unwrap())
                .unwrap();

        change_pubkey.tx.eth_signature = PackedEthSignature::deserialize_packed(
            &hex::decode("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026dbda09f107bd026f5d6ed4f02a0a81e257a2f5d6ed4f07b81dbda09f107bd026dbda09f107bd026f5d6ed4f0d4").unwrap(),
        ).ok();

        assert_eq!(
            hex::encode(change_pubkey.get_eth_witness()),
            "2a0a81e257a2f5d6ed4f07b81dbda09f107bd026dbda09f107bd026f5d6ed4f02a0a81e257a2f5d6ed4f07b81dbda09f107bd026dbda09f107bd026f5d6ed4f0d4"
        );
    }
}

#[cfg(test)]
pub mod tx_conversion_test {
    use super::*;

    // General configuration parameters for all types of operations
    const ACCOUNT_ID: u32 = 100;
    const TOKEN_ID: u16 = 5;
    const NONCE: u32 = 20;
    lazy_static! {
        static ref ALICE: Address =
            Address::from_str("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026").unwrap();
        static ref BOB: Address =
            Address::from_str("21abaed8712072e918632259780e587698ef58da").unwrap();
        static ref PK_HASH: PubKeyHash =
            PubKeyHash::from_hex("sync:3cfb9a39096d9e02b24187355f628f9a6331511b").unwrap();
        static ref AMOUNT: BigUint = BigUint::from(12345678u64);
        static ref FEE: BigUint = BigUint::from(1000000u32);
    }

    #[test]
    fn test_convert_to_bytes_change_pubkey() {
        let change_pubkey = ChangePubKey::new(
            ACCOUNT_ID,
            *ALICE,
            (*PK_HASH).clone(),
            TOKEN_ID,
            (*FEE).clone(),
            NONCE,
            None,
            None,
        );

        let bytes = change_pubkey.get_bytes();
        assert_eq!(hex::encode(bytes), "07000000642a0a81e257a2f5d6ed4f07b81dbda09f107bd0263cfb9a39096d9e02b24187355f628f9a6331511b00057d0300000014");
    }

    #[test]
    fn test_convert_to_bytes_transfer() {
        let transfer = Transfer::new(
            ACCOUNT_ID,
            *ALICE,
            *BOB,
            TOKEN_ID,
            (*AMOUNT).clone(),
            (*FEE).clone(),
            NONCE,
            None,
        );

        let bytes = transfer.get_bytes();
        assert_eq!(hex::encode(bytes), "05000000642a0a81e257a2f5d6ed4f07b81dbda09f107bd02621abaed8712072e918632259780e587698ef58da000500178c29c07d0300000014");
    }

    #[test]
    fn test_convert_to_bytes_forced_exit() {
        let forced_exit =
            ForcedExit::new(ACCOUNT_ID, *ALICE, TOKEN_ID, (*FEE).clone(), NONCE, None);

        let bytes = forced_exit.get_bytes();
        assert_eq!(
            hex::encode(bytes),
            "08000000642a0a81e257a2f5d6ed4f07b81dbda09f107bd02600057d0300000014"
        );
    }

    #[test]
    fn test_convert_to_bytes_withdraw() {
        let withdraw = Withdraw::new(
            ACCOUNT_ID,
            *ALICE,
            *BOB,
            TOKEN_ID,
            (*AMOUNT).clone(),
            (*FEE).clone(),
            NONCE,
            None,
        );

        let bytes = withdraw.get_bytes();
        assert_eq!(hex::encode(bytes), "03000000642a0a81e257a2f5d6ed4f07b81dbda09f107bd02621abaed8712072e918632259780e587698ef58da000500000000000000000000000000bc614e7d0300000014");
    }
}

#[test]
fn test_priority_op_from_valid_logs() {
    let valid_logs = [
        Log {
            address: Address::from_str("bd2ea2073d4efa1a82269800a362f889545983c2").unwrap(),
            topics: vec![H256::from_str(
                "d0943372c08b438a88d4b39d77216901079eda9ca59d45349841c099083b6830",
            )
            .unwrap()],
            data: Bytes(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 54, 97, 92, 243, 73, 215, 246, 52, 72, 145,
                177, 231, 202, 124, 114, 136, 63, 93, 192, 73, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                160, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 18, 133, 59, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 108,
                107, 147, 91, 139, 189, 64, 0, 0, 111, 183, 165, 210, 134, 53, 93, 80, 193, 119,
                133, 131, 237, 37, 53, 35, 227, 136, 205, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]),
            block_hash: Some(
                H256::from_str("1de24c5271c2a3ecdc8b56449b479e388a2390be481faad72d48799c93668c42")
                    .unwrap(),
            ),
            block_number: Some(1196475.into()),
            transaction_hash: Some(
                H256::from_str("5319d65d7a60a1544e4b17d2272f00b5d17a68dea4a0a92e40d046f98a8ed6c5")
                    .unwrap(),
            ),
            transaction_index: Some(0.into()),
            log_index: Some(0.into()),
            transaction_log_index: None,
            log_type: None,
            removed: Some(false),
        },
        Log {
            address: Address::from_str("bd2ea2073d4efa1a82269800a362f889545983c2").unwrap(),
            topics: vec![H256::from_str(
                "d0943372c08b438a88d4b39d77216901079eda9ca59d45349841c099083b6830",
            )
            .unwrap()],
            data: Bytes(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 54, 97, 92, 243, 73, 215, 246, 52, 72, 145,
                177, 231, 202, 124, 114, 136, 63, 93, 192, 73, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 26, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                160, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 18, 133, 223, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 42, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 37,
                184, 252, 127, 148, 119, 128, 0, 59, 187, 156, 57, 129, 3, 106, 206, 113, 189, 130,
                135, 229, 227, 157, 236, 165, 121, 1, 164, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]),
            block_hash: Some(
                H256::from_str("6054a4d29cda776d0493805cb8898e0659711532430b1af9844a48e67f5c794f")
                    .unwrap(),
            ),
            block_number: Some(1196639.into()),
            transaction_hash: Some(
                H256::from_str("4ab4002673c2b28853eebb00de588a2f8507d20078f29caef192d8b815acd379")
                    .unwrap(),
            ),
            transaction_index: Some(2.into()),
            log_index: Some(6.into()),
            transaction_log_index: None,
            log_type: None,
            removed: Some(false),
        },
    ];

    for event in valid_logs.iter() {
        let op = PriorityOp::try_from((*event).clone());

        assert!(op.is_ok());
    }
}
