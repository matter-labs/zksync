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
    tx::{ChangePubKey, ForcedExit, PackedEthSignature, TimeRange, Transfer, Withdraw},
    Log, PriorityOp,
};
use lazy_static::lazy_static;
use zksync_basic_types::{AccountId, Address, Nonce, TokenId, H256};

#[cfg(test)]
pub mod operations_test {
    use super::*;
    use crate::tx::{ChangePubKeyECDSAData, ChangePubKeyEthAuthData};

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
                token: TokenId(42),
                amount: BigUint::from(42u32),
            };
            let account_id = 42u32;

            DepositOp {
                priority_op,
                account_id: AccountId(account_id),
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
                AccountId(42),
                Address::from_str("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026").unwrap(),
                Address::from_str("21abaed8712072e918632259780e587698ef58da").unwrap(),
                TokenId(42),
                BigUint::from(42u32),
                BigUint::from(42u32),
                Nonce(42),
                Default::default(),
                None,
            );
            let (from, to) = (AccountId(1u32), AccountId(2u32));

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
                AccountId(42),
                Address::from_str("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026").unwrap(),
                Address::from_str("21abaed8712072e918632259780e587698ef58da").unwrap(),
                TokenId(42),
                BigUint::from(42u32),
                BigUint::from(42u32),
                Nonce(42),
                Default::default(),
                None,
            );
            let account_id = AccountId(42u32);

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
                account_id: AccountId(42),
                token: TokenId(42),
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
                AccountId(42),
                Address::from_str("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026").unwrap(),
                PubKeyHash::from_hex("sync:3cfb9a39096d9e02b24187355f628f9a6331511b").unwrap(),
                TokenId(42),
                BigUint::from(42u32),
                Nonce(42),
                Default::default(),
                None,
                Some(PackedEthSignature::deserialize_packed(
                    &hex::decode("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026dbda09f107bd026f5d6ed4f02a0a81e257a2f5d6ed4f07b81dbda09f107bd026dbda09f107bd026f5d6ed4f0d4").unwrap(),
                ).unwrap()),
            );
            let account_id = AccountId(42u32);

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
                AccountId(42),
                Address::from_str("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026").unwrap(),
                TokenId(42),
                BigUint::from(42u32),
                Nonce(42),
                Default::default(),
                None,
            );
            let target_account_id = AccountId(42u32);

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

        change_pubkey.tx.eth_auth_data = Some(ChangePubKeyEthAuthData::ECDSA(ChangePubKeyECDSAData {
            eth_signature: PackedEthSignature::deserialize_packed(
            &hex::decode("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026dbda09f107bd026f5d6ed4f02a0a81e257a2f5d6ed4f07b81dbda09f107bd026dbda09f107bd026f5d6ed4f0d4").unwrap(),
            ).expect("Hex signature deserialization"),
            batch_hash: H256::from([0x0u8; 32])
        }));

        assert_eq!(
            hex::encode(change_pubkey.get_eth_witness()),
            "002a0a81e257a2f5d6ed4f07b81dbda09f107bd026dbda09f107bd026f5d6ed4f02a0a81e257a2f5d6ed4f07b81dbda09f107bd026dbda09f107bd026f5d6ed4f0d4"
        );
    }
}

#[cfg(test)]
pub mod tx_conversion_test {
    use super::*;

    // General configuration parameters for all types of operations
    const ACCOUNT_ID: AccountId = AccountId(100);
    const TOKEN_ID: TokenId = TokenId(5);
    const NONCE: Nonce = Nonce(20);
    const VALID_FROM: u64 = 0;
    const VALID_UNTIL: u64 = 1612201680;
    lazy_static! {
        static ref ALICE: Address =
            Address::from_str("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026").unwrap();
        static ref BOB: Address =
            Address::from_str("21abaed8712072e918632259780e587698ef58da").unwrap();
        static ref PK_HASH: PubKeyHash =
            PubKeyHash::from_hex("sync:3cfb9a39096d9e02b24187355f628f9a6331511b").unwrap();
        static ref AMOUNT: BigUint = BigUint::from(12345678u64);
        static ref FEE: BigUint = BigUint::from(1000000u32);
        static ref TIME_RANGE: TimeRange = TimeRange::new(VALID_FROM, VALID_UNTIL);
    }

    #[test]
    fn test_convert_to_bytes_change_pubkey() {
        let change_pubkey = ChangePubKey::new(
            ACCOUNT_ID,
            *ALICE,
            *PK_HASH,
            TOKEN_ID,
            (*FEE).clone(),
            NONCE,
            *TIME_RANGE,
            None,
            None,
        );

        let bytes = change_pubkey.get_bytes();
        assert_eq!(hex::encode(bytes), "07000000642a0a81e257a2f5d6ed4f07b81dbda09f107bd0263cfb9a39096d9e02b24187355f628f9a6331511b00057d030000001400000000000000000000000060183ed0");
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
            *TIME_RANGE,
            None,
        );

        let bytes = transfer.get_bytes();
        assert_eq!(hex::encode(bytes), "05000000642a0a81e257a2f5d6ed4f07b81dbda09f107bd02621abaed8712072e918632259780e587698ef58da000500178c29c07d030000001400000000000000000000000060183ed0");
    }

    #[test]
    fn test_convert_to_bytes_forced_exit() {
        let forced_exit = ForcedExit::new(
            ACCOUNT_ID,
            *ALICE,
            TOKEN_ID,
            (*FEE).clone(),
            NONCE,
            *TIME_RANGE,
            None,
        );

        let bytes = forced_exit.get_bytes();
        assert_eq!(
            hex::encode(bytes),
            "08000000642a0a81e257a2f5d6ed4f07b81dbda09f107bd02600057d030000001400000000000000000000000060183ed0"
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
            *TIME_RANGE,
            None,
        );

        let bytes = withdraw.get_bytes();
        assert_eq!(hex::encode(bytes), "03000000642a0a81e257a2f5d6ed4f07b81dbda09f107bd02621abaed8712072e918632259780e587698ef58da000500000000000000000000000000bc614e7d030000001400000000000000000000000060183ed0");
    }
}

#[test]
fn test_priority_op_from_valid_logs() {
    let valid_logs = [
        Log {
            address: Address::from_str("aBC49f8a744b7b615994e1D42058c2D146B83389").unwrap(),
            topics: vec![H256::from_str(
                "d0943372c08b438a88d4b39d77216901079eda9ca59d45349841c099083b6830",
            )
            .unwrap()],
            data: Bytes(
                hex::decode(
                    "000000000000000000000000a61464658afeaf65cccaafd3a5\
                12b69a83b77618000000000000000000000000000000000000\
                00000000000000000000000000000000000000000000000000\
                00000000000000000000000000000000000000000100000000\
                00000000000000000000000000000000000000000000000000\
                0000a000000000000000000000000000000000000000000000\
                00000000000000000078000000000000000000000000000000\
                000000000000000000000000000000002b0100000000000100\
                000000000000000de0b6b3a7640000a61464658afeaf65ccca\
                afd3a512b69a83b77618000000000000000000000000000000\
                000000000000",
                )
                .expect("Event data parse"),
            ),
            block_hash: Some(
                H256::from_str("3a2116a8c2600a91df2ee940b94f19c29d59622ec1dafcdd0609697ffddf16d1")
                    .unwrap(),
            ),
            block_number: Some(1196475.into()),
            transaction_hash: Some(
                H256::from_str("52888e463aa4e8856970c783d6cf5f076f249d8e160db46f08a2edde4beb557e")
                    .unwrap(),
            ),
            transaction_index: Some(0.into()),
            log_index: Some(0.into()),
            transaction_log_index: None,
            log_type: None,
            removed: Some(false),
        },
        Log {
            address: Address::from_str("aBC49f8a744b7b615994e1D42058c2D146B83389").unwrap(),
            topics: vec![H256::from_str(
                "d0943372c08b438a88d4b39d77216901079eda9ca59d45349841c099083b6830",
            )
            .unwrap()],
            data: Bytes(
                hex::decode(
                    "000000000000000000000000a61464658afeaf65cccaafd3a5\
                                 12b69a83b77618000000000000000000000000000000000000\
                                 00000000000000000000000000030000000000000000000000\
                                 00000000000000000000000000000000000000000600000000\
                                 00000000000000000000000000000000000000000000000000\
                                 0000a000000000000000000000000000000000000000000000\
                                 00000000000000000087000000000000000000000000000000\
                                 000000000000000000000000000000002b0600000001a61464\
                                 658afeaf65cccaafd3a512b69a83b776180001000000000000\
                                 00000000000000000000000000000000000000000000000000000000000000",
                )
                .expect("Event data decode"),
            ),
            block_hash: Some(
                H256::from_str("a9b74339d253fdee7a9569f3892f9f63a55b8f7fed8e8334bf00f53b2d67b3a6")
                    .unwrap(),
            ),
            block_number: Some(1196639.into()),
            transaction_hash: Some(
                H256::from_str("fdc53efb32245f59009d10d7f67bfad5272a872a9aa539c2ce9cf1ed127750ad")
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
