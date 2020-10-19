use zksync_basic_types::H256;
use zksync_crypto::ff::Field;
use zksync_crypto::Fr;

use super::utils::*;
use crate::block::Block;

/// Checks that we cannot create a block with invalid block sizes provided.
#[test]
#[should_panic]
fn no_supported_block_size() {
    Block::new_from_available_block_sizes(
        0,
        Default::default(),
        0,
        vec![create_withdraw_tx()],
        (0, 0),
        &[0],
        1_000_000.into(),
        1_500_000.into(),
    );
}

/// Checks that the byte order is indeed big-endian.
#[test]
fn test_get_eth_encoded_root() {
    let block = Block::new(
        0,
        Fr::one(),
        0,
        vec![],
        (0, 0),
        1,
        1_000_000.into(),
        1_500_000.into(),
    );

    let mut bytes = [0u8; 32];
    let byte = bytes.last_mut().unwrap();
    *byte = 1;

    assert_eq!(block.get_eth_encoded_root(), H256::from(bytes));
}

#[test]
fn test_get_eth_public_data() {
    let mut block = Block::new(
        0,
        Fr::one(),
        0,
        vec![
            create_change_pubkey_tx(),
            create_full_exit_op(),
            create_withdraw_tx(),
        ],
        (0, 0),
        100,
        1_000_000.into(),
        1_500_000.into(),
    );

    let expected = {
        let mut data = vec![];
        for op in &block.block_transactions {
            data.extend(op.get_executed_op().unwrap().public_data());
        }
        data
    };

    let mut result = block.get_eth_public_data();
    // Skip the padding.
    result.truncate(expected.len());

    assert_eq!(result, expected);

    block.block_transactions = vec![];
    // Vec will be padded again.
    assert!(block.get_eth_public_data().iter().all(|&i| i == 0));
}

#[test]
fn test_get_eth_witness_data() {
    let operations = vec![
        create_change_pubkey_tx(),
        create_full_exit_op(),
        create_withdraw_tx(),
        create_change_pubkey_tx(),
    ];
    let change_pubkey_tx = &operations[0];
    let mut block = Block::new(
        0,
        Fr::one(),
        0,
        operations.clone(),
        (0, 0),
        100,
        1_000_000.into(),
        1_500_000.into(),
    );

    let witness = change_pubkey_tx
        .get_executed_op()
        .unwrap()
        .eth_witness()
        .unwrap();
    let used_bytes = witness.len() as u64;

    let expected = (
        [&witness[..], &witness[..]].concat(),
        vec![used_bytes, used_bytes],
    );

    assert_eq!(block.get_eth_witness_data(), expected);

    block.block_transactions.pop();
    let expected = (witness, vec![used_bytes]);
    assert_eq!(block.get_eth_witness_data(), expected);

    // Remove the last operation which has witness data.
    block.block_transactions.remove(0);
    assert!(block.get_eth_witness_data().0.is_empty());
}

#[test]
fn test_get_withdrawals_data() {
    let operations = vec![
        create_change_pubkey_tx(),
        create_full_exit_op(),
        create_withdraw_tx(),
    ];
    let mut block = Block::new(
        0,
        Fr::one(),
        0,
        operations.clone(),
        (0, 0),
        100,
        1_000_000.into(),
        1_500_000.into(),
    );

    let expected = {
        let mut data = vec![];
        for op in &operations[1..] {
            data.extend(op.get_executed_op().unwrap().withdrawal_data().unwrap());
        }
        data
    };

    assert_eq!(block.get_withdrawals_data(), expected);

    block.block_transactions.pop();
    assert!(!block.get_withdrawals_data().is_empty());

    block.block_transactions.pop();
    // No more corresponding operations left.
    assert!(block.get_withdrawals_data().is_empty());
}
