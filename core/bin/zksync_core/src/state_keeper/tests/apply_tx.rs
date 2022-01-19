use zksync_types::{AccountId, TokenId};

use super::utils::*;

/// Checks if withdrawal is processed correctly by the state_keeper.
#[test]
fn success() {
    let mut tester = StateKeeperTester::new(6, 1, 1);
    let old_pending_block = tester.state_keeper.pending_block.clone();
    let withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(0),
        AccountId(1),
        200u32,
        145u32,
        Default::default(),
    );
    let result = tester.state_keeper.apply_tx(&withdraw);
    let pending_block = tester.state_keeper.pending_block;

    assert!(result.is_included());
    assert!(pending_block.chunks_left < old_pending_block.chunks_left);
    assert_eq!(
        pending_block.pending_op_block_index,
        old_pending_block.pending_op_block_index + 1
    );
    assert!(!pending_block.account_updates.is_empty());
    assert!(!pending_block.success_operations.is_empty());
    assert!(!pending_block.collected_fees.is_empty());
}

/// Checks if fast withdrawal makes fast processing required.
#[test]
fn fast_withdrawal() {
    let mut tester = StateKeeperTester::new(6, 1, 1);
    let old_pending_block = tester.state_keeper.pending_block.clone();
    let withdraw = create_account_and_fast_withdrawal(
        &mut tester,
        TokenId(0),
        AccountId(1),
        200u32,
        145u32,
        Default::default(),
    );
    let result = tester.state_keeper.apply_tx(&withdraw);
    let pending_block = tester.state_keeper.pending_block;

    assert!(result.is_included());
    assert!(!old_pending_block.fast_processing_required);
    assert!(pending_block.fast_processing_required);
}

/// Checks if withdrawal that will fail is processed correctly.
#[test]
fn failure() {
    let mut tester = StateKeeperTester::new(6, 1, 1);
    let old_pending_block = tester.state_keeper.pending_block.clone();
    let withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(0),
        AccountId(1),
        100u32,
        145u32,
        Default::default(),
    );
    let result = tester.state_keeper.apply_tx(&withdraw);
    let pending_block = tester.state_keeper.pending_block;

    assert!(result.is_included());
    assert_eq!(pending_block.chunks_left, old_pending_block.chunks_left);
    assert_eq!(
        pending_block.pending_op_block_index,
        old_pending_block.pending_op_block_index
    );
    assert!(pending_block.account_updates.is_empty());
    assert!(!pending_block.failed_txs.is_empty());
    assert!(pending_block.collected_fees.is_empty());
}

/// Checks if processing withdrawal fails because of small number of chunks left in the block.
#[test]
fn not_enough_chunks() {
    let mut tester = StateKeeperTester::new(1, 1, 1);
    let withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(0),
        AccountId(1),
        200u32,
        145u32,
        Default::default(),
    );
    let result = tester.state_keeper.apply_tx(&withdraw);
    assert!(result.is_not_included());
}
