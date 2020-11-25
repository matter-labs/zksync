use std::convert::TryFrom;
use std::str::FromStr;

use zksync_basic_types::{Address, H256};

use web3::types::Bytes;

use super::*;

#[test]
fn test_try_from_valid_logs() {
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
