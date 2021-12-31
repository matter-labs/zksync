use chrono::Utc;
use num::BigUint;
use zksync_types::tx::TimeRange;
use zksync_types::{AccountId, SignedZkSyncTx, TokenId, Transfer, ZkSyncTx};

use super::utils::*;
use crate::state_keeper::utils::system_time_timestamp;

/// Checks if block sealing is done correctly by sealing a block
/// with 1 priority_op, 1 succeeded tx, 1 failed tx
#[tokio::test]
async fn seal_pending_block() {
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

    assert!(tester.state_keeper.apply_tx(&good_withdraw).is_included());
    assert!(tester.state_keeper.apply_tx(&bad_withdraw).is_included());
    assert!(tester
        .state_keeper
        .apply_priority_op(&deposit)
        .is_included());

    let old_updates_len = tester.state_keeper.pending_block.account_updates.len();
    tester.state_keeper.seal_pending_block().await;

    assert!(tester.state_keeper.pending_block.failed_txs.is_empty());
    assert!(tester
        .state_keeper
        .pending_block
        .success_operations
        .is_empty());
    assert!(tester.state_keeper.pending_block.collected_fees.is_empty());
    assert!(tester.state_keeper.pending_block.account_updates.is_empty());
    assert_eq!(tester.state_keeper.pending_block.chunks_left, 20);

    let (block, updates) = tester.unwrap_sealed_update().await;

    let collected_fees = tester
        .state_keeper
        .state
        .get_account(tester.fee_collector)
        .unwrap()
        .get_balance(TokenId(0));
    assert_eq!(block.block.block_transactions.len(), 3);
    assert_eq!(collected_fees, BigUint::from(1u32));
    assert_eq!(block.block.processed_priority_ops, (0, 1));
    assert_eq!(
        tester.state_keeper.pending_block.number,
        block.block.block_number + 1
    );
    assert_eq!(
        updates.account_updates.len(),
        // + 1 here is for the update corresponding to collected fee
        old_updates_len - updates.first_update_order_id + 1
    );
}

/// Checks if block storing is done correctly by storing a block
/// with 1 priority_op, 1 succeeded tx, 1 failed tx
#[tokio::test]
async fn store_pending_block() {
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

    assert!(tester.state_keeper.apply_tx(&good_withdraw).is_included());
    assert!(tester.state_keeper.apply_tx(&bad_withdraw).is_included());
    assert!(tester
        .state_keeper
        .apply_priority_op(&deposit)
        .is_included());

    tester.state_keeper.store_pending_block().await;

    let (block, _) = tester.unwrap_pending_update().await;

    assert_eq!(block.number, tester.state_keeper.pending_block.number);
    assert_eq!(
        block.chunks_left,
        tester.state_keeper.pending_block.chunks_left
    );
    assert_eq!(
        block.unprocessed_priority_op_before,
        tester
            .state_keeper
            .pending_block
            .unprocessed_priority_op_before
    );
    assert_eq!(
        block.pending_block_iteration,
        tester.state_keeper.pending_block.pending_block_iteration
    );
    assert_eq!(
        block.success_operations.len(),
        tester.state_keeper.pending_block.success_operations.len()
    );
    assert_eq!(
        block.failed_txs.len(),
        tester.state_keeper.pending_block.failed_txs.len()
    );
}

/// Checks that if transaction was executed correctly in the pending block,
/// it will not be skipped when the block is restored even if the *current* timestamp
/// does not allow it (but timestamp in the pending block allowed it at the moment of
/// execution).
#[tokio::test]
async fn correctly_restore_pending_block_timestamp() {
    let mut tester = StateKeeperTester::new(20, 3, 3);

    // Amount of time transaction should be valid.
    const VALID_UNTIL_DIFF: u64 = 1;
    let valid_until = system_time_timestamp() + VALID_UNTIL_DIFF;

    let token_id = TokenId(0);
    let account_id = AccountId(1);
    let balance = 200u32;
    let transfer_amount = 145u32;

    // We manually create an account, since we will re-use it in the new state keeper to re-initialize it.
    let (account, sk) = tester.add_account(account_id);
    tester.set_balance(account_id, token_id, balance);

    let good_transfer = {
        let time_range = TimeRange::new(0, valid_until);

        let transfer = Transfer::new_signed(
            account_id,
            account.address,
            account.address,
            token_id,
            transfer_amount.into(),
            BigUint::from(1u32),
            account.nonce,
            time_range,
            &sk,
        )
        .unwrap();
        SignedZkSyncTx {
            tx: ZkSyncTx::Transfer(Box::new(transfer)),
            eth_sign_data: None,
            created_at: Utc::now(),
        }
    };
    assert!(tester.state_keeper.apply_tx(&good_transfer).is_included());

    tester.state_keeper.store_pending_block().await;

    let previous_stored_account_updates = tester.state_keeper.pending_block.stored_account_updates;
    assert_ne!(
        previous_stored_account_updates, 0,
        "There should be more than 0 stored account updates"
    );

    let (pending_block, _) = tester.unwrap_pending_update().await;

    assert_eq!(
        pending_block.number,
        tester.state_keeper.pending_block.number
    );
    assert_eq!(pending_block.success_operations.len(), 1);
    assert_eq!(pending_block.failed_txs.len(), 0);

    // Sleep until tx is invalid.
    while system_time_timestamp() <= valid_until {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Create new state keeper and re-execute transaction.
    let mut tester = StateKeeperTester::new(20, 3, 3);

    // Insert the account in its original form into state keeper.
    tester
        .state_keeper
        .state
        .insert_account(account_id, account.clone());
    tester.set_balance(account_id, token_id, balance);

    // Run initialize function and process the pending block.
    // This operation should successfully execute the transfer even though now it's past the time it's valid.
    tester.state_keeper.initialize(Some(pending_block.clone()));

    assert_eq!(
        tester.state_keeper.pending_block.number, pending_block.number,
        "Incorrect block number in state keeper"
    );

    assert_eq!(
        tester.state_keeper.pending_block.success_operations.len(),
        1,
        "There should be 1 successful tx"
    );
    assert_eq!(
        tester.state_keeper.pending_block.failed_txs.len(),
        0,
        "There should be 0 failed txs"
    );

    // Check that `stored_account_updates` represent actually processed updates.
    assert_eq!(
        tester.state_keeper.pending_block.stored_account_updates, previous_stored_account_updates,
        "Stored account updates were restored incorrectly"
    );

    // Just in case try to execute a *new* transaction with the same timestamp.
    // It should still be valid, because the timestamp was restored from the pending block.
    let withdraw_with_same_valid_until = create_account_and_withdrawal(
        &mut tester,
        TokenId(2),
        AccountId(2),
        300u32,
        145u32,
        TimeRange::new(0, valid_until),
    );
    tester
        .state_keeper
        .apply_tx(&withdraw_with_same_valid_until)
        .assert_included("Tx was not applied");

    assert_eq!(
        tester.state_keeper.pending_block.success_operations.len(),
        2,
        "Tx with the same valid_until as for previous transaction should've been processed"
    );
}
