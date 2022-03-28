use zksync_mempool::ProposedBlock;
use zksync_types::gas_counter::{GasCounter, VerifyCost, TX_GAS_LIMIT};
use zksync_types::{
    mempool::SignedTxVariant, mempool::SignedTxsBatch, AccountId, ExecutedOperations, TokenId,
};

use super::utils::*;

/// Checks if processing withdrawal fails because the gas limit is reached.
/// This sends 46 withdrawals (very inefficient, but all constants in GasCounter are hardcoded, so we can't do much here).
#[test]
fn gas_limit_reached() {
    let withdrawals_number = (TX_GAS_LIMIT - VerifyCost::base_cost().as_u64() * 130 / 100)
        / (VerifyCost::WITHDRAW_COST * 130 / 100);
    let mut tester = StateKeeperTester::new(6 * withdrawals_number as usize, 1, 1);
    for i in 1..=withdrawals_number {
        let withdrawal = create_account_and_withdrawal(
            &mut tester,
            TokenId(0),
            AccountId(i as u32),
            200u32,
            145u32,
            Default::default(),
        );
        let result = tester.state_keeper.apply_tx(&withdrawal);
        if i <= withdrawals_number {
            assert!(
                result.is_included(),
                "i: {}, withdrawals: {}",
                i,
                withdrawals_number
            )
        } else {
            assert!(
                result.is_not_included(),
                "i: {}, withdrawals: {}",
                i,
                withdrawals_number
            )
        }
    }
}

/// Checks that execution of failed transaction shouldn't change gas count.
#[tokio::test]
async fn gas_count_change() {
    let mut tester = StateKeeperTester::new(50, 5, 5);
    let initial_gas_count = tester
        .state_keeper
        .pending_block
        .gas_counter
        .commit_gas_limit();

    // Create withdraw which will fail.
    let withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(0),
        AccountId(1),
        200u32,
        300u32,
        Default::default(),
    );
    let result = tester.state_keeper.apply_tx(&withdraw);

    assert!(result.is_included());
    // Check that gas count shouldn't change
    assert_eq!(
        initial_gas_count,
        tester
            .state_keeper
            .pending_block
            .gas_counter
            .commit_gas_limit()
    );

    // Create two transfers which will fail.
    let first_transfer =
        create_account_and_transfer(&mut tester, TokenId(0), AccountId(2), 200u32, 300u32);
    let second_transfer =
        create_account_and_transfer(&mut tester, TokenId(0), AccountId(3), 200u32, 300u32);
    let proposed_block = ProposedBlock {
        txs: vec![SignedTxVariant::Batch(SignedTxsBatch {
            txs: vec![first_transfer, second_transfer],
            batch_id: 1,
            eth_signatures: Vec::new(),
        })],
        priority_ops: Vec::new(),
    };
    tester
        .state_keeper
        .execute_proposed_block(proposed_block)
        .await;
    // Check that gas count shouldn't change.
    assert_eq!(
        initial_gas_count,
        tester
            .state_keeper
            .pending_block
            .gas_counter
            .commit_gas_limit()
    );

    // Create correct transfer.
    let third_transfer =
        create_account_and_transfer(&mut tester, TokenId(0), AccountId(4), 200u32, 100u32);

    let result = tester.state_keeper.apply_tx(&third_transfer);

    assert!(result.is_included());
    // Check that gas count should increase.
    assert!(
        initial_gas_count
            < tester
                .state_keeper
                .pending_block
                .gas_counter
                .commit_gas_limit()
    );
}

/// Calculates count of withdrawals that fit into block gas limit.
fn withdrawals_fit_into_block() -> u32 {
    let mut tester = StateKeeperTester::new(1000, 1000, 1000);

    let withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(0),
        AccountId(1),
        20u32,
        10u32,
        Default::default(),
    );
    let op = tester
        .state_keeper
        .state
        .zksync_tx_to_zksync_op(withdraw.tx)
        .unwrap();
    let mut count = 0;
    let mut gas_counter = GasCounter::new();
    while gas_counter.add_op(&op).is_ok() {
        count += 1;
    }
    count
}

/// Checks that block seals after reaching gas limit.
#[tokio::test]
async fn gas_limit_sealing() {
    let mut tester = StateKeeperTester::new(1000, 1000, 1000);

    let withdrawals_count = withdrawals_fit_into_block();

    // Create (withdrawals_count + 1) withdrawal
    let txs: Vec<_> = (0..=withdrawals_count)
        .map(|i| {
            let withdraw = create_account_and_withdrawal(
                &mut tester,
                TokenId(0),
                AccountId(i + 1),
                20u32,
                10u32,
                Default::default(),
            );
            SignedTxVariant::Tx(withdraw)
        })
        .collect();
    let last_withdraw = match txs.last().unwrap() {
        SignedTxVariant::Tx(tx) => tx.clone(),
        _ => panic!("Tx was expected"),
    };
    let proposed_block = ProposedBlock {
        txs,
        priority_ops: Vec::new(),
    };
    tester
        .state_keeper
        .execute_proposed_block(proposed_block)
        .await;

    // Checks that one block is sealed and pending block has only the last withdrawal.
    tester.assert_sealed().await;
    tester
        .assert_pending_with(|block| {
            assert_eq!(block.success_operations.len(), 1);
            match &block.success_operations[0] {
                ExecutedOperations::Tx(tx) => {
                    assert_eq!(tx.signed_tx.tx.hash(), last_withdraw.tx.hash());
                }
                _ => panic!("Tx was expected"),
            }
        })
        .await;
}

/// Checks that batch that doesn't fit into gas limit is processed correctly.
#[tokio::test]
async fn batch_gas_limit() {
    let mut tester = StateKeeperTester::new(1000, 1000, 1000);
    let withdrawals_count = withdrawals_fit_into_block();

    // Create (withdrawals_count + 1) withdrawal
    let txs: Vec<_> = (0..=withdrawals_count)
        .map(|i| {
            create_account_and_withdrawal(
                &mut tester,
                TokenId(0),
                AccountId(i + 1),
                20u32,
                10u32,
                Default::default(),
            )
        })
        .collect();

    let proposed_block = ProposedBlock {
        txs: vec![SignedTxVariant::Batch(SignedTxsBatch {
            txs,
            batch_id: 1,
            eth_signatures: Vec::new(),
        })],
        priority_ops: Vec::new(),
    };
    // Execute big batch.
    tester
        .state_keeper
        .execute_proposed_block(proposed_block)
        .await;
    tester
        .assert_pending_with(|block| {
            assert_eq!(block.failed_txs.len() as u32, withdrawals_count + 1);
            let expected_fail_reason =
                Some("Amount of gas required to process batch is too big".to_string());
            for tx in block.failed_txs {
                assert_eq!(tx.fail_reason, expected_fail_reason);
            }
        })
        .await;
}
