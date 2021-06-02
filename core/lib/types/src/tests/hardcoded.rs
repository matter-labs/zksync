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
        ChangePubKeyOp, DepositOp, ForcedExitOp, FullExitOp, NoopOp, SwapOp, TransferOp,
        TransferToNewOp, WithdrawNFTOp, WithdrawOp,
    },
    priority_ops::{Deposit, FullExit},
    tx::{
        ChangePubKey, ForcedExit, MintNFT, Order, PackedEthSignature, Swap, TimeRange, Transfer,
        Withdraw, WithdrawNFT,
    },
    Log, PriorityOp,
};
use once_cell::sync::Lazy;
use zksync_basic_types::{AccountId, Address, Nonce, TokenId, H256};

#[cfg(test)]
pub mod operations_test {
    use super::*;
    use crate::tx::{ChangePubKeyECDSAData, ChangePubKeyEthAuthData};
    use crate::{MintNFT, MintNFTOp};
    use zksync_crypto::params::MIN_NFT_TOKEN_ID;

    // Public data parameters, using them we can restore `ZkSyncOp`.
    const NOOP_PUBLIC_DATA: &str = "00000000000000000000";
    const DEPOSIT_PUBLIC_DATA: &str = "010000002a0000002a0000000000000000000000000000002a21abaed8712072e918632259780e587698ef58da000000000000000000000000000000";
    const TRANSFER_TO_NEW_PUBLIC_DATA: &str = "02000000010000002a000000054021abaed8712072e918632259780e587698ef58da0000000205400000000000000000000000000000000000000000";
    const WITHDRAW_PUBLIC_DATA: &str =
        "030000002a0000002a0000000000000000000000000000002a054021abaed8712072e918632259780e587698ef58da00000000000000000000000000";
    const TRANSFER_PUBLIC_DATA: &str = "05000000010000002a0000000200000005400540";
    const FULL_EXIT_PUBLIC_DATA: &str = "060000002a2a0a81e257a2f5d6ed4f07b81dbda09f107bd0260000002a000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
    const CHANGE_PUBKEY_PUBLIC_DATA: &str = "070000002a3cfb9a39096d9e02b24187355f628f9a6331511b2a0a81e257a2f5d6ed4f07b81dbda09f107bd0260000002a0000002a05400000000000";
    const FORCED_EXIT_PUBLIC_DATA: &str = "080000002a0000002a0000002a0000000000000000000000000000000005402a0a81e257a2f5d6ed4f07b81dbda09f107bd026000000000000000000";
    const SWAP_PUBLIC_DATA: &str = "0b000000050000000600000007000000080000002a00000007000000010000002d00000012200000001b2005800200000000";
    const MINT_NFT_PUBLIC_DATA: &str = "090000000a0000000b0000000000000000000000000000000000000000000000000000000000000000000000000140000000";
    const WITHDRAW_NFT_PUBLIC_DATA: &str = "0a0000002a0000002b21abaed8712072e918632259780e587698ef58da00000000000000000000000000000000000000000000000000000000000000000000000021abaed8712072e918632259780e587698ef58da000100000000002a05400000000000";

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
    fn test_public_data_conversions_withdraw_nft() {
        let expected_op = {
            let tx = WithdrawNFT::new(
                AccountId(42),
                Address::from_str("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026").unwrap(),
                Address::from_str("21abaed8712072e918632259780e587698ef58da").unwrap(),
                TokenId(MIN_NFT_TOKEN_ID),
                TokenId(42),
                BigUint::from(42u32),
                Nonce(42),
                Default::default(),
                None,
            );
            let creator_account_id = AccountId(43u32);

            WithdrawNFTOp {
                tx,
                creator_id: creator_account_id,
                creator_address: Address::from_str("21abaed8712072e918632259780e587698ef58da")
                    .unwrap(),
                content_hash: Default::default(),
                serial_id: 0,
            }
        };

        assert_eq!(
            hex::encode(expected_op.get_public_data()),
            WITHDRAW_NFT_PUBLIC_DATA
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
                creator_account_id: None,
                creator_address: None,
                serial_id: None,
                content_hash: None,
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
    fn test_public_data_conversions_swap() {
        let expected_op = {
            let tx = Swap::new(
                AccountId(42),
                Address::random(),
                Nonce(43),
                (
                    Order {
                        account_id: AccountId(5),
                        nonce: Nonce(123),
                        recipient_address: Address::random(),
                        token_buy: TokenId(1),
                        token_sell: TokenId(7),
                        amount: BigUint::from(0u8),
                        price: (BigUint::from(1u8), BigUint::from(2u8)),
                        time_range: TimeRange::new(0, 1 << 31),
                        signature: Default::default(),
                    },
                    Order {
                        account_id: AccountId(7),
                        nonce: Nonce(100),
                        recipient_address: Address::random(),
                        token_buy: TokenId(7),
                        token_sell: TokenId(1),
                        amount: BigUint::from(12345u32),
                        price: (BigUint::from(2u8), BigUint::from(1u8)),
                        time_range: TimeRange::new(0, 1 << 31),
                        signature: Default::default(),
                    },
                ),
                (BigUint::from(145u32), BigUint::from(217u32)),
                BigUint::from(44u32),
                TokenId(45),
                None,
            );

            SwapOp {
                tx,
                submitter: AccountId(42),
                accounts: (AccountId(5), AccountId(7)),
                recipients: (AccountId(6), AccountId(8)),
            }
        };

        assert_eq!(hex::encode(expected_op.get_public_data()), SWAP_PUBLIC_DATA);
    }

    #[test]
    fn test_public_data_conversions_mint_nft() {
        let expected_op = MintNFTOp {
            tx: MintNFT::new(
                AccountId(10),
                Address::default(),
                H256::default(),
                Address::default(),
                BigUint::from(10u32),
                TokenId(0),
                Nonce(0),
                None,
            ),
            creator_account_id: AccountId(10),
            recipient_account_id: AccountId(11),
        };
        assert_eq!(
            hex::encode(expected_op.get_public_data()),
            MINT_NFT_PUBLIC_DATA
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
            "0121abaed8712072e918632259780e587698ef58da0000002a0000000000000000000000000000002a"
        );
        assert_eq!(
            hex::encode(forced_exit.get_withdrawal_data()),
            "012a0a81e257a2f5d6ed4f07b81dbda09f107bd0260000002a00000000000000000000000000000000"
        );
        assert_eq!(hex::encode(full_exit.get_withdrawal_data()), "002a0a81e257a2f5d6ed4f07b81dbda09f107bd0260000002a0000000000000000000000000000000000000000");
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
    const ACCOUNT_ID_2: AccountId = AccountId(200);
    const ACCOUNT_ID_3: AccountId = AccountId(300);
    const TOKEN_ID: TokenId = TokenId(5);
    const TOKEN_ID_2: TokenId = TokenId(6);
    const FEE_TOKEN_ID: TokenId = TokenId(18);
    const NONCE: Nonce = Nonce(20);
    const NONCE_2: Nonce = Nonce(30);
    const NONCE_3: Nonce = Nonce(40);
    const VALID_FROM: u64 = 0;
    const VALID_UNTIL: u64 = 1612201680;

    static ALICE: Lazy<Address> =
        Lazy::new(|| Address::from_str("2a0a81e257a2f5d6ed4f07b81dbda09f107bd026").unwrap());
    static BOB: Lazy<Address> =
        Lazy::new(|| Address::from_str("21abaed8712072e918632259780e587698ef58da").unwrap());
    static CARL: Lazy<Address> =
        Lazy::new(|| Address::from_str("002b598a1fc2f0d8240fbd8b13131b9eab0165a3").unwrap());
    static PK_HASH: Lazy<PubKeyHash> = Lazy::new(|| {
        PubKeyHash::from_hex("sync:3cfb9a39096d9e02b24187355f628f9a6331511b").unwrap()
    });
    static AMOUNT: Lazy<BigUint> = Lazy::new(|| BigUint::from(12345678u64));
    static AMOUNT_2: Lazy<BigUint> = Lazy::new(|| BigUint::from(87654321u64));
    static FEE: Lazy<BigUint> = Lazy::new(|| BigUint::from(1000000u32));
    static TIME_RANGE: Lazy<TimeRange> = Lazy::new(|| TimeRange::new(VALID_FROM, VALID_UNTIL));

    #[test]
    fn test_convert_to_bytes_swap() {
        let swap = Swap::new(
            ACCOUNT_ID,
            *ALICE,
            NONCE,
            (
                Order {
                    account_id: ACCOUNT_ID_2,
                    nonce: NONCE_2,
                    recipient_address: *BOB,
                    token_buy: TOKEN_ID,
                    token_sell: TOKEN_ID_2,
                    amount: AMOUNT.clone(),
                    price: (&*AMOUNT + BigUint::from(2u8), AMOUNT_2.clone()),
                    time_range: *TIME_RANGE,
                    signature: Default::default(),
                },
                Order {
                    account_id: ACCOUNT_ID_3,
                    nonce: NONCE_3,
                    recipient_address: *CARL,
                    token_buy: TOKEN_ID_2,
                    token_sell: TOKEN_ID,
                    amount: AMOUNT_2.clone(),
                    price: (&*AMOUNT_2 + BigUint::from(2u8), AMOUNT.clone()),
                    time_range: *TIME_RANGE,
                    signature: Default::default(),
                },
            ),
            (AMOUNT.clone(), AMOUNT_2.clone()),
            FEE.clone(),
            FEE_TOKEN_ID,
            None,
        );

        let bytes = swap.get_bytes();
        assert_eq!(hex::encode(bytes), "f401000000642a0a81e257a2f5d6ed4f07b81dbda09f107bd026000000146f01000000c821abaed8712072e918632259780e587698ef58da0000001e0000000600000005000000000000000000000000bc6150000000000000000000000005397fb100178c29c000000000000000000000000060183ed06f010000012c002b598a1fc2f0d8240fbd8b13131b9eab0165a3000000280000000500000006000000000000000000000005397fb3000000000000000000000000bc614e00a72ff62000000000000000000000000060183ed0000000127d0300178c29c000a72ff620");
    }

    #[test]
    fn test_convert_to_bytes_withdraw_nft() {
        let withdrwa_nft = WithdrawNFT::new(
            ACCOUNT_ID,
            *ALICE,
            *ALICE,
            TOKEN_ID,
            TOKEN_ID,
            (*FEE).clone(),
            NONCE,
            Default::default(),
            None,
        );
        let bytes = withdrwa_nft.get_bytes();
        assert_eq!(hex::encode(bytes), "f501000000642a0a81e257a2f5d6ed4f07b81dbda09f107bd0262a0a81e257a2f5d6ed4f07b81dbda09f107bd02600000005000000057d03000000140000000000000000ffffffffffffffff");
    }

    #[test]
    fn test_convert_to_bytes_mint_nft() {
        let mint_nft = MintNFT::new(
            ACCOUNT_ID,
            *ALICE,
            H256::default(),
            *BOB,
            (*FEE).clone(),
            TOKEN_ID,
            NONCE,
            None,
        );
        let bytes = mint_nft.get_bytes();
        assert_eq!(hex::encode(bytes), "f601000000642a0a81e257a2f5d6ed4f07b81dbda09f107bd026000000000000000000000000000000000000000000000000000000000000000021abaed8712072e918632259780e587698ef58da000000057d0300000014");
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
        assert_eq!(hex::encode(bytes), "f801000000642a0a81e257a2f5d6ed4f07b81dbda09f107bd0263cfb9a39096d9e02b24187355f628f9a6331511b000000057d030000001400000000000000000000000060183ed0");
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
        assert_eq!(hex::encode(bytes), "fa01000000642a0a81e257a2f5d6ed4f07b81dbda09f107bd02621abaed8712072e918632259780e587698ef58da0000000500178c29c07d030000001400000000000000000000000060183ed0");
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
            "f701000000642a0a81e257a2f5d6ed4f07b81dbda09f107bd026000000057d030000001400000000000000000000000060183ed0"
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
        assert_eq!(hex::encode(bytes), "fc01000000642a0a81e257a2f5d6ed4f07b81dbda09f107bd02621abaed8712072e918632259780e587698ef58da0000000500000000000000000000000000bc614e7d030000001400000000000000000000000060183ed0");
    }
}

#[test]
#[ignore]
// TODO restore this test, generate correct log
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
                000000000000000000000000000000002d01000000000000000100\
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
