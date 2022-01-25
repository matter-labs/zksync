use zksync_types::TokenId;

use super::utils::*;

/// Checks if deposit is processed correctly by the state_keeper.
#[test]
fn success() {
    let mut tester = StateKeeperTester::new(8, 1, 1);
    let old_pending_block = tester.state_keeper.pending_block.clone();
    let deposit = create_deposit(TokenId(0), 145u32);
    let result = tester.state_keeper.apply_priority_op(&deposit);
    let pending_block = tester.state_keeper.pending_block;
    assert!(result.is_included());
    assert!(pending_block.chunks_left < old_pending_block.chunks_left);
    assert_eq!(
        pending_block.pending_op_block_index,
        old_pending_block.pending_op_block_index + 1
    );
    assert!(!pending_block.account_updates.is_empty());
    assert!(!pending_block.success_operations.is_empty());
    assert_eq!(pending_block.unprocessed_priority_op_current, 1);
}

/// Checks if processing deposit fails because of small number of chunks left in the block.
#[test]
fn not_enough_chunks() {
    let mut tester = StateKeeperTester::new(1, 1, 1);
    let deposit = create_deposit(TokenId(0), 1u32);
    let result = tester.state_keeper.apply_priority_op(&deposit);
    assert!(result.is_not_included());
}
