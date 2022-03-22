use chrono::Utc;
use zksync_mempool::ProposedBlock;
use zksync_types::{
    mempool::SignedTxVariant, AccountId, BlockNumber, Nonce, SignedZkSyncTx, TokenId, Transfer,
    ZkSyncTx,
};

use super::utils::*;

/// Checks if executing a proposed_block with just enough chunks is done correctly
/// and checks if number of chunks left is correct after each operation.
#[tokio::test]
async fn just_enough_chunks() {
    let mut tester = StateKeeperTester::new(8, 3, 3);

    // First batch
    apply_batch_with_two_transfers(&mut tester).await;
    tester
        .assert_pending_with(|block| assert_eq!(block.chunks_left, 4))
        .await;

    // Second batch
    apply_batch_with_two_transfers(&mut tester).await;
    tester
        .assert_sealed_with(|block| assert_eq!(block.block_transactions.len(), 4))
        .await;
}

/// Checks if executing a proposed_block is done correctly
/// when two batches don`t fit into one block.
/// Also, checks if number of chunks left is correct after each operation.
#[tokio::test]
async fn chunks_to_fit_three_transfers_2_2_1() {
    let mut tester = StateKeeperTester::new(6, 3, 3);

    // First batch
    apply_batch_with_two_transfers(&mut tester).await;
    tester
        .assert_pending_with(|block| assert_eq!(block.chunks_left, 2))
        .await;

    // Second batch
    apply_batch_with_two_transfers(&mut tester).await;
    tester
        .assert_sealed_with(|block| assert_eq!(block.block_transactions.len(), 2))
        .await;
    tester
        .assert_pending_with(|block| assert_eq!(block.chunks_left, 2))
        .await;

    // Single tx
    apply_single_transfer(&mut tester).await;
    tester
        .assert_sealed_with(|block| assert_eq!(block.block_transactions.len(), 3))
        .await;
}

/// Checks if executing a proposed_block is done correctly
/// when two single txs and one batch don`t fit into one block.
/// Also, checks if number of chunks left is correct after each operation.
#[tokio::test]
async fn chunks_to_fit_three_transfers_1_1_2_1() {
    let mut tester = StateKeeperTester::new(6, 3, 3);

    // First single tx
    apply_single_transfer(&mut tester).await;
    tester
        .assert_pending_with(|block| assert_eq!(block.chunks_left, 4))
        .await;

    // Second single tx
    apply_single_transfer(&mut tester).await;
    tester
        .assert_pending_with(|block| assert_eq!(block.chunks_left, 2))
        .await;

    // First batch
    apply_batch_with_two_transfers(&mut tester).await;
    tester
        .assert_sealed_with(|block| assert_eq!(block.block_transactions.len(), 2))
        .await;
    tester
        .assert_pending_with(|block| assert_eq!(block.chunks_left, 2))
        .await;

    // Last single tx
    apply_single_transfer(&mut tester).await;
    tester
        .assert_sealed_with(|block| assert_eq!(block.block_transactions.len(), 3))
        .await;
}

/// Checks if executing a small proposed_block is done correctly.
#[tokio::test]
async fn small() {
    let mut tester = StateKeeperTester::new(20, 3, 3);
    let good_withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(0),
        AccountId(1),
        200u32,
        145u32,
        Default::default(),
    );
    let bad_withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(2),
        AccountId(2),
        100u32,
        145u32,
        Default::default(),
    );
    let deposit = create_deposit(TokenId(0), 12u32);
    let proposed_block = ProposedBlock {
        txs: vec![
            SignedTxVariant::Tx(good_withdraw),
            SignedTxVariant::Tx(bad_withdraw),
        ],
        priority_ops: vec![deposit],
    };
    let pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
    tester
        .state_keeper
        .execute_proposed_block(proposed_block)
        .await;
    tester.assert_pending().await;
    assert_eq!(
        pending_block_iteration + 1,
        tester.state_keeper.pending_block.pending_block_iteration
    );
}

/// Checks if executing a proposed_block is done correctly
/// There are more chunks than one can fit in 1 block,
/// so 1 block should get sealed in the process.
#[tokio::test]
async fn few_chunks() {
    let mut tester = StateKeeperTester::new(12, 3, 3);
    let good_withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(0),
        AccountId(1),
        200u32,
        145u32,
        Default::default(),
    );
    let bad_withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(2),
        AccountId(2),
        100u32,
        145u32,
        Default::default(),
    );
    let deposit = create_deposit(TokenId(0), 12u32);
    let proposed_block = ProposedBlock {
        txs: vec![
            SignedTxVariant::Tx(good_withdraw),
            SignedTxVariant::Tx(bad_withdraw),
        ],
        priority_ops: vec![deposit],
    };
    tester
        .state_keeper
        .execute_proposed_block(proposed_block)
        .await;
    tester.assert_sealed().await;
    tester.assert_pending().await;
}

/// Checks if executing a proposed_block is done correctly
/// max_iterations == 0, so the block should get sealed, not stored.
#[tokio::test]
async fn few_iterations() {
    let mut tester = StateKeeperTester::new(20, 0, 0);
    let good_withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(0),
        AccountId(1),
        200u32,
        145u32,
        Default::default(),
    );
    let bad_withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(2),
        AccountId(2),
        100u32,
        145u32,
        Default::default(),
    );
    let deposit = create_deposit(TokenId(0), 12u32);
    let proposed_block = ProposedBlock {
        txs: vec![
            SignedTxVariant::Tx(good_withdraw),
            SignedTxVariant::Tx(bad_withdraw),
        ],
        priority_ops: vec![deposit],
    };
    tester
        .state_keeper
        .execute_proposed_block(proposed_block)
        .await;

    // Proposed block is *always* sent, even if block was sealed.
    tester.assert_sealed().await;
}

/// Checks that fast withdrawal causes block to be sealed faster.
#[tokio::test]
async fn fast_withdrawal() {
    const MAX_ITERATIONS: usize = 100;
    const FAST_ITERATIONS: usize = 0; // Seal block right after fast withdrawal.

    let mut tester = StateKeeperTester::new(6, MAX_ITERATIONS, FAST_ITERATIONS);
    let withdraw = create_account_and_fast_withdrawal(
        &mut tester,
        TokenId(0),
        AccountId(1),
        200u32,
        145u32,
        Default::default(),
    );

    let proposed_block = ProposedBlock {
        priority_ops: Vec::new(),
        txs: vec![withdraw.into()],
    };

    tester
        .state_keeper
        .execute_proposed_block(proposed_block)
        .await;

    // We should receive the next block, since it must be sealed right after.
    tester.assert_sealed().await;
}

/// Checks the following things:
/// 1. if proposed block is empty, no pending block is yielded from the state keeper.
/// 2. if there were no successful operations in the block, pending block iteration is not incremented after empty or rejected-only updates.
/// 3. if there were successful operations in the block, pending block iteration is incremented after each `execute_proposed_block` call.
#[tokio::test]
async fn pending_block_updates() {
    let mut tester = StateKeeperTester::new(20, 5, 5);

    // --- Phase 1: Empty pending block, empty update. ---

    // Check that empty update with empty pending block doesn't increment the iteration.
    let proposed_block = ProposedBlock {
        txs: vec![],
        priority_ops: vec![],
    };

    tester
        .state_keeper
        .execute_proposed_block(proposed_block)
        .await;

    // There should be no pending block yielded.
    tester.assert_empty().await;

    // No successful operations in the pending block => no increment.
    let pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
    assert_eq!(pending_block_iteration, 0);

    // --- Phase 2: Empty pending block, only failed tx in update. ---

    // Then send the block with the bad transaction only
    let bad_withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(2),
        AccountId(1),
        100u32,
        145u32,
        Default::default(),
    );
    let proposed_block = ProposedBlock {
        txs: vec![SignedTxVariant::Tx(bad_withdraw)],
        priority_ops: vec![],
    };

    tester
        .state_keeper
        .execute_proposed_block(proposed_block)
        .await;

    // Pending block should be created.
    tester.assert_pending().await;

    // Iteration should still not be incremented.
    let pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
    assert_eq!(pending_block_iteration, 0);

    // --- Phase 3: Empty pending block, successful tx in update. ---

    // First, create some block with successfull operation.
    let good_withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(2),
        AccountId(1),
        200u32,
        145u32,
        Default::default(),
    );
    let proposed_block = ProposedBlock {
        txs: vec![SignedTxVariant::Tx(good_withdraw)],
        priority_ops: vec![],
    };

    let pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
    tester
        .state_keeper
        .execute_proposed_block(proposed_block)
        .await;

    // Pending block should be created.
    tester.assert_pending().await;

    // Iteration should be incremented.
    let new_pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
    assert_eq!(new_pending_block_iteration, pending_block_iteration + 1);

    // --- Phase 4: Successful tx in pending block, failed tx in update. ---

    // Then send the block with the bad transaction only.
    let bad_withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(2),
        AccountId(1),
        100u32,
        145u32,
        Default::default(),
    );
    let proposed_block = ProposedBlock {
        txs: vec![SignedTxVariant::Tx(bad_withdraw)],
        priority_ops: vec![],
    };

    let pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
    tester
        .state_keeper
        .execute_proposed_block(proposed_block)
        .await;

    // Pending block should be created.
    tester.assert_pending().await;

    // Iteration should still be incremented.
    let new_pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
    assert_eq!(new_pending_block_iteration, pending_block_iteration + 1);

    // --- Phase 5: Successful tx in pending block, empty update. ---

    // Finally, execute an empty block.
    let proposed_block = ProposedBlock {
        txs: vec![],
        priority_ops: vec![],
    };

    let pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
    tester
        .state_keeper
        .execute_proposed_block(proposed_block)
        .await;

    // There should be no pending block yielded.
    tester.assert_empty().await;

    // Iteration should still be incremented even after an empty block: there was a successful operation earlier.
    let new_pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
    assert_eq!(new_pending_block_iteration, pending_block_iteration + 1);
}

/// Checks that only the difference between two states of a pending block is transmitted
/// to the committer.
#[tokio::test]
async fn pending_block_diff() {
    let mut tester = StateKeeperTester::new(20, 5, 5);

    let good_withdraw_1 = create_account_and_withdrawal(
        &mut tester,
        TokenId(0),
        AccountId(1),
        200u32,
        145u32,
        Default::default(),
    );
    let bad_withdraw_1 = create_account_and_withdrawal(
        &mut tester,
        TokenId(2),
        AccountId(2),
        100u32,
        145u32,
        Default::default(),
    );
    let proposed_block_1 = ProposedBlock {
        txs: vec![
            SignedTxVariant::Tx(good_withdraw_1.clone()),
            SignedTxVariant::Tx(bad_withdraw_1.clone()),
        ],
        priority_ops: vec![],
    };

    let good_withdraw_2 = create_account_and_withdrawal(
        &mut tester,
        TokenId(0),
        AccountId(3),
        200u32,
        145u32,
        Default::default(),
    );
    let bad_withdraw_2 = create_account_and_withdrawal(
        &mut tester,
        TokenId(2),
        AccountId(4),
        100u32,
        145u32,
        Default::default(),
    );
    let proposed_block_2 = ProposedBlock {
        txs: vec![
            SignedTxVariant::Tx(good_withdraw_2.clone()),
            SignedTxVariant::Tx(bad_withdraw_2.clone()),
        ],
        priority_ops: vec![],
    };

    tester
        .state_keeper
        .execute_proposed_block(proposed_block_1)
        .await;
    tester
        .assert_pending_with(|block| {
            assert_eq!(*block.number, 1); // It's the first block.
            assert_eq!(block.success_operations.len(), 1);
            assert_eq!(
                block.success_operations[0]
                    .get_executed_tx()
                    .unwrap()
                    .signed_tx
                    .hash(),
                good_withdraw_1.hash()
            );

            assert_eq!(block.failed_txs.len(), 1);
            assert_eq!(block.failed_txs[0].signed_tx.hash(), bad_withdraw_1.hash());
        })
        .await;

    // Now we execute the next proposed block and expect that only the diff between `pending_block_2` and
    // `pending_block_1` will be sent.
    tester
        .state_keeper
        .execute_proposed_block(proposed_block_2)
        .await;
    tester
        .assert_pending_with(|block| {
            assert_eq!(*block.number, 1); // It still should be the first block.
            assert_eq!(block.success_operations.len(), 1);
            assert_eq!(
                block.success_operations[0]
                    .get_executed_tx()
                    .unwrap()
                    .signed_tx
                    .hash(),
                good_withdraw_2.hash()
            );

            assert_eq!(block.failed_txs.len(), 1);
            assert_eq!(block.failed_txs[0].signed_tx.hash(), bad_withdraw_2.hash());
        })
        .await;
}

/// Checks that a transaction with a valid timestamp accepted by the statekeeper
/// and transaction with an invalid timestamp failed.
#[tokio::test]
async fn transfers_with_different_timestamps() {
    let mut tester = StateKeeperTester::new(20, 5, 5);

    let token_id = TokenId(0);
    let account_from_id = AccountId(1);
    let account_to_id = AccountId(2);
    let balance = 999u32;
    let fee = 0u32;
    let (account_from, sk_from) = tester.add_account(account_from_id);
    let (account_to, _sk_to) = tester.add_account(account_to_id);
    tester.set_balance(account_from_id, token_id, balance);

    let correct_transfer = Transfer::new_signed(
        account_from_id,
        account_from.address,
        account_to.address,
        token_id,
        balance.into(),
        fee.into(),
        Nonce(0),
        Default::default(),
        &sk_from,
    )
    .unwrap();

    let mut premature_transfer = correct_transfer.clone();
    if let Some(time_range) = premature_transfer.time_range.as_mut() {
        time_range.valid_from = u64::max_value();
    }

    let mut belated_transfer = correct_transfer.clone();
    if let Some(time_range) = belated_transfer.time_range.as_mut() {
        time_range.valid_until = 0;
    }

    let correct_transfer = SignedZkSyncTx {
        tx: ZkSyncTx::Transfer(Box::new(correct_transfer)),
        eth_sign_data: None,
        created_at: Utc::now(),
    };
    let premature_transfer = SignedZkSyncTx {
        tx: ZkSyncTx::Transfer(Box::new(premature_transfer)),
        eth_sign_data: None,
        created_at: Utc::now(),
    };
    let belated_transfer = SignedZkSyncTx {
        tx: ZkSyncTx::Transfer(Box::new(belated_transfer)),
        eth_sign_data: None,
        created_at: Utc::now(),
    };
    let proposed_block = ProposedBlock {
        txs: vec![
            SignedTxVariant::Tx(premature_transfer.clone()),
            SignedTxVariant::Tx(belated_transfer.clone()),
            SignedTxVariant::Tx(correct_transfer.clone()),
        ],
        priority_ops: vec![],
    };

    tester
        .state_keeper
        .execute_proposed_block(proposed_block)
        .await;
    tester
        .assert_pending_with(|block| {
            assert_eq!(block.number, BlockNumber(1)); // It's the first block.

            assert_eq!(block.success_operations.len(), 1);
            assert_eq!(
                block.success_operations[0]
                    .get_executed_tx()
                    .unwrap()
                    .signed_tx
                    .hash(),
                correct_transfer.hash()
            );

            assert_eq!(block.failed_txs.len(), 2);
            assert_eq!(
                block.failed_txs[0].signed_tx.hash(),
                premature_transfer.hash()
            );
            assert_eq!(
                block.failed_txs[1].signed_tx.hash(),
                belated_transfer.hash()
            );
        })
        .await;
}
