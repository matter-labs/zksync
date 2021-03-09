// Local uses
use self::mock::{
    concurrent_eth_sender, create_signed_tx, default_eth_parameters, default_eth_sender,
    restored_eth_sender,
};
use super::{transactions::TxCheckOutcome, ETHSender, TxCheckMode};
use web3::types::U64;
use zksync_eth_client::ethereum_gateway::ExecutedTxStatus;

const EXPECTED_WAIT_TIME_BLOCKS: u64 = 30;
const WAIT_CONFIRMATIONS: u64 = 3;

pub mod mock;
mod test_data;

/// Basic test that `ETHSender` creation does not panic and initializes correctly.
#[tokio::test]
async fn basic_test() {
    let eth_sender = default_eth_sender().await;

    // Check that there are no unconfirmed operations by default.
    assert!(eth_sender.ongoing_ops.is_empty());
}

/// Checks that deadline block is chosen according to the expected policy.
#[tokio::test]
async fn deadline_block() {
    let eth_sender = default_eth_sender().await;

    assert_eq!(eth_sender.get_deadline_block(0), EXPECTED_WAIT_TIME_BLOCKS);
    assert_eq!(
        eth_sender.get_deadline_block(10),
        10 + EXPECTED_WAIT_TIME_BLOCKS
    );
}

/// Checks that received transaction response is reduced to the
/// `TxCheckOutcome` correctly.
///
/// Here we check every possible output of the `check_transaction_state` method.
#[tokio::test]
async fn transaction_state() {
    let mut eth_sender = default_eth_sender().await;
    let current_block = eth_sender
        .ethereum
        .get_mock()
        .unwrap()
        .block_number()
        .await
        .unwrap()
        .as_u64();
    let deadline_block = eth_sender.get_deadline_block(current_block);
    let operations = vec![
        test_data::commit_blocks_operation(0), // Will be committed.
        test_data::commit_blocks_operation(1), // Will be pending because of not enough confirmations.
        test_data::commit_blocks_operation(2), // Will be failed.
        test_data::commit_blocks_operation(3), // Will be failed and pending (not enough confirmations).
        test_data::commit_blocks_operation(4), // Will be stuck.
        test_data::commit_blocks_operation(5), // Will be pending due no response.
    ];
    let mut eth_operations = Vec::with_capacity(operations.len());

    for (eth_op_id, op) in operations.iter().enumerate() {
        eth_operations.push(
            create_signed_tx(
                eth_op_id as i64,
                &eth_sender,
                op.clone(),
                deadline_block,
                eth_op_id as i64,
            )
            .await,
        )
    }

    // Committed operation.
    let committed_response = ExecutedTxStatus {
        confirmations: WAIT_CONFIRMATIONS,
        success: true,
        receipt: None,
    };
    eth_sender
        .ethereum
        .get_mut_mock()
        .unwrap()
        .add_execution(&eth_operations[0].used_tx_hashes[0], &committed_response)
        .await;

    // Pending operation.
    let pending_response = ExecutedTxStatus {
        confirmations: WAIT_CONFIRMATIONS - 1,
        success: true,
        receipt: None,
    };
    eth_sender
        .ethereum
        .get_mut_mock()
        .unwrap()
        .add_execution(&eth_operations[1].used_tx_hashes[0], &pending_response)
        .await;

    // Failed operation.
    let failed_response = ExecutedTxStatus {
        confirmations: WAIT_CONFIRMATIONS,
        success: false,
        receipt: Some(Default::default()),
    };
    eth_sender
        .ethereum
        .get_mut_mock()
        .unwrap()
        .add_execution(&eth_operations[2].used_tx_hashes[0], &failed_response)
        .await;

    // Pending failed operation.
    let pending_failed_response = ExecutedTxStatus {
        confirmations: WAIT_CONFIRMATIONS - 1,
        success: false,
        receipt: Some(Default::default()),
    };
    eth_sender
        .ethereum
        .get_mut_mock()
        .unwrap()
        .add_execution(
            &eth_operations[3].used_tx_hashes[0],
            &pending_failed_response,
        )
        .await;

    // Committed operation.
    assert_eq!(
        eth_sender
            .check_transaction_state(
                TxCheckMode::Latest,
                &eth_operations[0],
                eth_operations[0].used_tx_hashes[0],
                current_block + committed_response.confirmations,
            )
            .await
            .unwrap(),
        TxCheckOutcome::Committed
    );

    // Pending operation (no enough confirmations).
    assert_eq!(
        eth_sender
            .check_transaction_state(
                TxCheckMode::Latest,
                &eth_operations[1],
                eth_operations[1].used_tx_hashes[0],
                current_block + pending_response.confirmations,
            )
            .await
            .unwrap(),
        TxCheckOutcome::Pending
    );

    // Failed operation.
    assert_eq!(
        eth_sender
            .check_transaction_state(
                TxCheckMode::Latest,
                &eth_operations[2],
                eth_operations[2].used_tx_hashes[0],
                current_block + failed_response.confirmations,
            )
            .await
            .unwrap(),
        TxCheckOutcome::Failed(Default::default())
    );

    // Pending failed operation should be considered as pending.
    assert_eq!(
        eth_sender
            .check_transaction_state(
                TxCheckMode::Latest,
                &eth_operations[3],
                eth_operations[3].used_tx_hashes[0],
                current_block + pending_failed_response.confirmations,
            )
            .await
            .unwrap(),
        TxCheckOutcome::Pending
    );

    // Stuck operation.
    assert_eq!(
        eth_sender
            .check_transaction_state(
                TxCheckMode::Latest,
                &eth_operations[4],
                eth_operations[4].used_tx_hashes[0],
                current_block + EXPECTED_WAIT_TIME_BLOCKS,
            )
            .await
            .unwrap(),
        TxCheckOutcome::Stuck
    );

    // Pending operation (no response yet).
    assert_eq!(
        eth_sender
            .check_transaction_state(
                TxCheckMode::Latest,
                &eth_operations[5],
                eth_operations[5].used_tx_hashes[0],
                current_block + EXPECTED_WAIT_TIME_BLOCKS - 1,
            )
            .await
            .unwrap(),
        TxCheckOutcome::Pending
    );

    // Pending old operation should be considered stuck.
    assert_eq!(
        eth_sender
            .check_transaction_state(
                TxCheckMode::Old,
                &eth_operations[5],
                eth_operations[5].used_tx_hashes[0],
                current_block + EXPECTED_WAIT_TIME_BLOCKS - 1,
            )
            .await
            .unwrap(),
        TxCheckOutcome::Stuck
    );
}

/// Test for a normal `ETHSender` workflow:
/// - we send the two sequential operations (commit, verify, execute);
/// - they are successfully committed to the Ethereum;
/// - notification is sent after `execute` operation is committed.
#[tokio::test]
async fn operation_commitment_workflow() {
    let mut eth_sender = default_eth_sender().await;

    // In this test we will run one commit blocks operation, one publish proof blocks onchain operation
    // and execute blocks operation and should obtain a notification about the operation being completed in the end.
    let aggregated_operations = vec![
        test_data::commit_blocks_operation(0),
        test_data::publish_proof_blocks_onchain_operations(0),
        test_data::execute_blocks_operations(0),
    ];

    for (eth_op_id, aggregated_operation) in aggregated_operations.iter().enumerate() {
        let nonce = eth_op_id as i64;

        // Send an operation to `ETHSender`.
        eth_sender
            .db
            .send_aggregated_operation(aggregated_operation.clone())
            .await
            .unwrap();

        // Retrieve it there and then process.
        eth_sender.load_new_operations().await.unwrap();

        eth_sender.proceed_next_operations().await;

        // Now we should see that transaction is stored in the database and sent to the Ethereum.
        let deadline_block = eth_sender.get_deadline_block(
            eth_sender
                .ethereum
                .get_mock()
                .unwrap()
                .block_number()
                .await
                .unwrap()
                .as_u64(),
        );
        let mut expected_tx = create_signed_tx(
            eth_op_id as i64,
            &eth_sender,
            aggregated_operation.clone(),
            deadline_block,
            nonce,
        )
        .await;
        expected_tx.id = eth_op_id as i64; // We have to set the ID manually.

        eth_sender.db.assert_stored(&expected_tx).await;

        eth_sender
            .ethereum
            .get_mock()
            .unwrap()
            .assert_sent(&expected_tx.used_tx_hashes[0].as_bytes().to_vec())
            .await;

        // Increment block, make the transaction look successfully executed, and process the
        // operation again.
        eth_sender
            .ethereum
            .get_mut_mock()
            .unwrap()
            .add_successfull_execution(expected_tx.used_tx_hashes[0], WAIT_CONFIRMATIONS)
            .await;

        eth_sender.proceed_next_operations().await;

        // Check that operation is confirmed.
        expected_tx.confirmed = true;
        expected_tx.final_hash = Some(expected_tx.used_tx_hashes[0]);
        eth_sender.db.assert_confirmed(&expected_tx).await;
    }
}

/// A simple scenario for a stuck transaction:
/// - A transaction is sent to the Ethereum.
/// - It is not processed after some blocks.
/// - `ETHSender` creates a new transaction with increased gas.
/// - This transaction is completed successfully.
#[tokio::test]
async fn stuck_transaction() {
    let mut eth_sender = default_eth_sender().await;

    // Workflow for the test is similar to `operation_commitment_workflow`.
    let aggregated_operation = test_data::commit_blocks_operation(0);
    // Send an operation to `ETHSender`.
    eth_sender
        .db
        .send_aggregated_operation(aggregated_operation.clone())
        .await
        .unwrap();

    eth_sender.load_new_operations().await.unwrap();
    eth_sender.proceed_next_operations().await;

    let eth_op_id = 0;
    let nonce = 0;
    let deadline_block = eth_sender.get_deadline_block(
        eth_sender
            .ethereum
            .get_mock()
            .unwrap()
            .block_number()
            .await
            .unwrap()
            .as_u64(),
    );
    let mut stuck_tx = create_signed_tx(
        eth_op_id,
        &eth_sender,
        aggregated_operation.clone(),
        deadline_block,
        nonce,
    )
    .await;

    let block_number = U64::from(
        eth_sender
            .ethereum
            .get_mock()
            .unwrap()
            .block_number()
            .await
            .unwrap()
            .as_u64()
            + EXPECTED_WAIT_TIME_BLOCKS,
    );
    // Skip some blocks and expect sender to send a new tx.
    eth_sender
        .ethereum
        .get_mut_mock()
        .unwrap()
        .set_block_number(block_number)
        .await
        .unwrap();
    eth_sender.proceed_next_operations().await;

    // Check that new transaction is sent (and created based on the previous stuck tx).
    let expected_sent_tx = eth_sender
        .create_supplement_tx(
            eth_sender.get_deadline_block(
                eth_sender
                    .ethereum
                    .get_mock()
                    .unwrap()
                    .block_number()
                    .await
                    .unwrap()
                    .as_u64(),
            ),
            &mut stuck_tx,
        )
        .await
        .unwrap();
    eth_sender.db.assert_stored(&stuck_tx).await;
    eth_sender
        .ethereum
        .get_mut_mock()
        .unwrap()
        .assert_sent(&expected_sent_tx.hash.as_bytes().to_vec())
        .await;

    // Increment block, make the transaction look successfully executed, and process the
    // operation again.
    eth_sender
        .ethereum
        .get_mut_mock()
        .unwrap()
        .add_successfull_execution(stuck_tx.used_tx_hashes[1], WAIT_CONFIRMATIONS)
        .await;
    eth_sender.proceed_next_operations().await;

    // Check that operation is confirmed (we set the final hash to the second sent tx).
    stuck_tx.confirmed = true;
    stuck_tx.final_hash = Some(stuck_tx.used_tx_hashes[1]);
    eth_sender.db.assert_confirmed(&stuck_tx).await;
}

/// This test verifies that with multiple operations received all-together,
/// their order is respected and no processing of the next operation is started until
/// the previous one is committed.
///
/// This test includes all three operation types (commit, verify and execute).
#[tokio::test]
async fn operations_order() {
    let mut eth_sender = default_eth_sender().await;

    // We send multiple the operations at once to the channel.
    let operations_count = 3;

    let commit_operations = &test_data::COMMIT_BLOCKS_OPERATIONS[..operations_count];
    let verify_operations = &test_data::PUBLISH_PROOF_BLOCKS_ONCHAIN_OPERATIONS[..operations_count];
    let execute_operations = &test_data::EXECUTE_BLOCKS_OPERATIONS[..operations_count];

    // Also we create the list of expected transactions.
    let mut expected_txs = Vec::new();

    // Create expected txs from all the operations.
    // Since we create 3 operations at each cycle iteration,
    // the logic of ID calculating is (i * 3), (i * 3 + 1), (i * 3 + 2).
    // On the first iteration the indices 0, 1 and 2 will be taken, then it
    // will be 3, 4 and 5, etc.
    let operation_iterator = commit_operations
        .iter()
        .zip(verify_operations)
        .zip(execute_operations);
    for (idx, ((commit_operation, verify_operation), execute_operation)) in
        operation_iterator.enumerate()
    {
        // Create the commit operation.
        let start_block = 1 + WAIT_CONFIRMATIONS * (idx * 3) as u64;
        let deadline_block = eth_sender.get_deadline_block(start_block);
        let eth_op_idx = (idx * 3) as i64;
        let nonce = eth_op_idx;

        let commit_op_tx = create_signed_tx(
            eth_op_idx,
            &eth_sender,
            commit_operation.clone(),
            deadline_block,
            nonce,
        )
        .await;

        expected_txs.push(commit_op_tx);
        // Send commit operation
        eth_sender
            .db
            .send_aggregated_operation(commit_operation.clone())
            .await
            .unwrap();

        // Create the verify operation, as by priority it will be processed right after `commit`.
        let start_block = 1 + WAIT_CONFIRMATIONS * (idx * 3 + 1) as u64;
        let deadline_block = eth_sender.get_deadline_block(start_block);
        let eth_op_idx = (idx * 3 + 1) as i64;
        let nonce = eth_op_idx;

        let verify_op_tx = create_signed_tx(
            eth_op_idx,
            &eth_sender,
            verify_operation.clone(),
            deadline_block,
            nonce,
        )
        .await;

        expected_txs.push(verify_op_tx);
        // Send verify operation
        eth_sender
            .db
            .send_aggregated_operation(verify_operation.clone())
            .await
            .unwrap();

        // Create the withdraw operation.
        let start_block = 1 + WAIT_CONFIRMATIONS * (idx * 3 + 2) as u64;
        let deadline_block = eth_sender.get_deadline_block(start_block);
        let eth_op_idx = (idx * 3 + 2) as i64;
        let nonce = eth_op_idx;

        let execute_op_tx = create_signed_tx(
            eth_op_idx,
            &eth_sender,
            execute_operation.clone(),
            deadline_block,
            nonce,
        )
        .await;

        expected_txs.push(execute_op_tx);
        // Send execute operation
        eth_sender
            .db
            .send_aggregated_operation(execute_operation.clone())
            .await
            .unwrap();
    }

    eth_sender.load_new_operations().await.unwrap();

    // Then we go through the operations and check that the order of operations is preserved.
    for mut tx in expected_txs.into_iter() {
        let current_tx_hash = tx.used_tx_hashes[0];

        eth_sender.proceed_next_operations().await;

        // Check that current expected tx is stored.
        eth_sender.db.assert_stored(&tx).await;
        eth_sender
            .ethereum
            .get_mock()
            .unwrap()
            .assert_sent(&current_tx_hash.as_bytes().to_vec())
            .await;

        // Mark the tx as successfully
        eth_sender
            .ethereum
            .get_mut_mock()
            .unwrap()
            .add_successfull_execution(current_tx_hash, WAIT_CONFIRMATIONS)
            .await;
        eth_sender.proceed_next_operations().await;

        // Update the fields in the tx and check if it's confirmed.
        tx.confirmed = true;
        tx.final_hash = Some(current_tx_hash);
        eth_sender.db.assert_confirmed(&tx).await;
    }
}

/// Check that upon a transaction failure the incident causes a panic by default.
#[tokio::test]
#[should_panic(expected = "Cannot operate after unexpected TX failure")]
async fn transaction_failure() {
    let mut eth_sender = default_eth_sender().await;

    // Workflow for the test is similar to `operation_commitment_workflow`.
    let aggregated_operation = test_data::commit_blocks_operation(0);
    eth_sender
        .db
        .send_aggregated_operation(aggregated_operation.clone())
        .await
        .unwrap();

    let eth_op_id = 0;
    let nonce = 0;
    let deadline_block = eth_sender.get_deadline_block(
        eth_sender
            .ethereum
            .get_mock()
            .unwrap()
            .block_number()
            .await
            .unwrap()
            .as_u64(),
    );
    let failing_tx = create_signed_tx(
        eth_op_id,
        &eth_sender,
        aggregated_operation.clone(),
        deadline_block,
        nonce,
    )
    .await;

    eth_sender.load_new_operations().await.unwrap();
    eth_sender.proceed_next_operations().await;

    eth_sender
        .ethereum
        .get_mut_mock()
        .unwrap()
        .add_failed_execution(&failing_tx.used_tx_hashes[0], WAIT_CONFIRMATIONS)
        .await;
    eth_sender.proceed_next_operations().await;
}

/// Check that after recovering state with several non-processed operations
/// they will be processed normally.
#[tokio::test]
async fn restore_state() {
    let (stored_eth_operations, aggregated_operations, unprocessed_operations) = {
        // This `eth_sender` is required to generate the input only.
        let eth_sender = default_eth_sender().await;

        // Aggregated operations for which Ethereum transactions have been created but have not yet been confirmed.
        let processed_commit_op = test_data::commit_blocks_operation(0);
        let processed_verify_op = test_data::publish_proof_blocks_onchain_operations(0);
        let processed_execute_op = test_data::execute_blocks_operations(0);

        let deadline_block = eth_sender.get_deadline_block(1);
        let commit_op_tx = create_signed_tx(
            0,
            &eth_sender,
            processed_commit_op.clone(),
            deadline_block,
            0,
        )
        .await;

        let deadline_block = eth_sender.get_deadline_block(1 + WAIT_CONFIRMATIONS);
        let verify_op_tx = create_signed_tx(
            1,
            &eth_sender,
            processed_verify_op.clone(),
            deadline_block,
            1,
        )
        .await;

        let deadline_block = eth_sender.get_deadline_block(1 + 2 * WAIT_CONFIRMATIONS);
        let execute_op_tx = create_signed_tx(
            2,
            &eth_sender,
            processed_execute_op.clone(),
            deadline_block,
            2,
        )
        .await;

        let stored_eth_operations = vec![commit_op_tx, verify_op_tx, execute_op_tx];

        // Aggregated operations that have not yet been processed.
        let unprocessed_commit_op = test_data::commit_blocks_operation(1);
        let unprocessed_verify_op = test_data::publish_proof_blocks_onchain_operations(1);
        let unprocessed_execute_op = test_data::execute_blocks_operations(1);

        // All aggregated operations must be in the database even after server restart.
        let aggregated_operations = vec![
            processed_commit_op,
            processed_verify_op,
            processed_execute_op,
            unprocessed_commit_op,
            unprocessed_verify_op,
            unprocessed_execute_op.clone(),
        ];
        // Aggregated operations from the table `eth_unprocessed_aggregated_ops` are deleted after the operation is added to the queue,
        // therefore, after restarting the server, it may contain not all really unprocessed operations.
        let unprocessed_operations = vec![unprocessed_execute_op];

        (
            stored_eth_operations,
            aggregated_operations,
            unprocessed_operations,
        )
    };

    let mut eth_parameters = default_eth_parameters();
    eth_parameters.last_committed_block = 1;
    eth_parameters.last_verified_block = 1;
    eth_parameters.last_executed_block = 1;

    let mut eth_sender = restored_eth_sender(
        stored_eth_operations,
        aggregated_operations.clone(),
        unprocessed_operations,
        eth_parameters,
    )
    .await;

    eth_sender.load_new_operations().await.unwrap();

    for (eth_op_id, aggregated_operation) in aggregated_operations.iter().enumerate() {
        // Note that we DO NOT send an operation to `ETHSender` and neither receive it.

        // We do process operations restored from the DB though.
        // The rest of this test is the same as in `operation_commitment_workflow`.
        eth_sender.proceed_next_operations().await;

        let deadline_block = eth_sender.get_deadline_block(
            eth_sender
                .ethereum
                .get_mock()
                .unwrap()
                .block_number()
                .await
                .unwrap()
                .as_u64(),
        );
        let nonce = eth_op_id as i64;
        let mut expected_tx = create_signed_tx(
            eth_op_id as i64,
            &eth_sender,
            aggregated_operation.clone(),
            deadline_block,
            nonce,
        )
        .await;
        expected_tx.id = eth_op_id as i64;

        eth_sender.db.assert_stored(&expected_tx).await;

        eth_sender
            .ethereum
            .get_mut_mock()
            .unwrap()
            .add_successfull_execution(expected_tx.used_tx_hashes[0], WAIT_CONFIRMATIONS)
            .await;
        eth_sender.proceed_next_operations().await;

        expected_tx.confirmed = true;
        expected_tx.final_hash = Some(expected_tx.used_tx_hashes[0]);
        eth_sender.db.assert_confirmed(&expected_tx).await;
    }
}

/// Checks that even after getting the first transaction stuck and sending the next
/// one, confirmation for the first (stuck) transaction is processed and leads
/// to the operation commitment.
#[tokio::test]
async fn confirmations_independence() {
    // Workflow in the test is the same as in `stuck_transaction`, except for the fact
    // that confirmation is obtained for the stuck transaction instead of the latter one.

    let mut eth_sender = default_eth_sender().await;

    let aggregated_operation = test_data::commit_blocks_operation(0);
    eth_sender
        .db
        .send_aggregated_operation(aggregated_operation.clone())
        .await
        .unwrap();

    eth_sender.load_new_operations().await.unwrap();
    eth_sender.proceed_next_operations().await;

    let eth_op_id = 0;
    let nonce = 0;
    let deadline_block = eth_sender.get_deadline_block(
        eth_sender
            .ethereum
            .get_mock()
            .unwrap()
            .block_number()
            .await
            .unwrap()
            .as_u64(),
    );
    let mut stuck_tx = create_signed_tx(
        eth_op_id,
        &eth_sender,
        aggregated_operation.clone(),
        deadline_block,
        nonce,
    )
    .await;

    let block_number = U64::from(
        eth_sender
            .ethereum
            .get_mock()
            .unwrap()
            .block_number()
            .await
            .unwrap()
            .as_u64()
            + EXPECTED_WAIT_TIME_BLOCKS,
    );
    eth_sender
        .ethereum
        .get_mut_mock()
        .unwrap()
        .set_block_number(block_number)
        .await
        .unwrap();
    eth_sender.proceed_next_operations().await;

    let next_tx = eth_sender
        .create_supplement_tx(
            eth_sender.get_deadline_block(
                eth_sender
                    .ethereum
                    .get_mock()
                    .unwrap()
                    .block_number()
                    .await
                    .unwrap()
                    .as_u64(),
            ),
            &mut stuck_tx,
        )
        .await
        .unwrap();
    eth_sender.db.assert_stored(&stuck_tx).await;
    eth_sender
        .ethereum
        .get_mut_mock()
        .unwrap()
        .assert_sent(&next_tx.hash.as_bytes().to_vec())
        .await;

    // Add a confirmation for a *stuck* transaction.
    eth_sender
        .ethereum
        .get_mut_mock()
        .unwrap()
        .add_successfull_execution(stuck_tx.used_tx_hashes[0], WAIT_CONFIRMATIONS)
        .await;
    eth_sender.proceed_next_operations().await;

    // Check that operation is confirmed (we set the final hash to the *first* sent tx).
    stuck_tx.confirmed = true;
    stuck_tx.final_hash = Some(stuck_tx.used_tx_hashes[0]);
    eth_sender.db.assert_confirmed(&stuck_tx).await;
}

/// This test is the same as `operations_order`, but configures ETH sender
/// to use 3 transactions in flight, and checks that they are being sent concurrently.
#[tokio::test]
async fn concurrent_operations_order() {
    const MAX_TXS_IN_FLIGHT: u64 = 3;
    let mut eth_sender = concurrent_eth_sender(MAX_TXS_IN_FLIGHT).await;

    // We send multiple the operations at once to the channel.
    let operations_count = 3;
    let commit_operations = &test_data::COMMIT_BLOCKS_OPERATIONS[..operations_count];
    let verify_operations = &test_data::PUBLISH_PROOF_BLOCKS_ONCHAIN_OPERATIONS[..operations_count];
    let execute_operations = &test_data::EXECUTE_BLOCKS_OPERATIONS[..operations_count];

    // Also we create the list of expected transactions.
    let mut expected_txs = Vec::new();

    // Create expected txs from all the operations.
    // Since we create 3 operations at each cycle iteration,
    // the logic of ID calculating is (i * 3), (i * 3 + 1), (i * 3 + 2).
    // On the first iteration the indices 0, 1 and 2 will be taken, then it
    // will be 3, 4 and 5, etc.
    let operation_iterator = commit_operations
        .iter()
        .zip(verify_operations)
        .zip(execute_operations);
    for (idx, ((commit_operation, verify_operation), execute_operation)) in
        operation_iterator.enumerate()
    {
        // Commit/verify/execute transactions from one iteration will be sent concurrently,
        // thus the deadline block is the same for them.
        let start_block = 1 + WAIT_CONFIRMATIONS * (idx * 3) as u64;
        let deadline_block = eth_sender.get_deadline_block(start_block);

        // Create the commit operation.
        let eth_op_idx = (idx * 3) as i64;
        let nonce = eth_op_idx;

        let commit_op_tx = create_signed_tx(
            eth_op_idx,
            &eth_sender,
            commit_operation.clone(),
            deadline_block,
            nonce,
        )
        .await;

        expected_txs.push(commit_op_tx);

        // Create the verify operation, as by priority it will be processed right after `commit`.
        let eth_op_idx = (idx * 3 + 1) as i64;
        let nonce = eth_op_idx;

        let verify_op_tx = create_signed_tx(
            eth_op_idx,
            &eth_sender,
            verify_operation.clone(),
            deadline_block,
            nonce,
        )
        .await;

        expected_txs.push(verify_op_tx);

        // Create the execute operation, as by priority it will be processed right after `verify`.
        let eth_op_idx = (idx * 3 + 2) as i64;
        let nonce = eth_op_idx;

        let execute_op_tx = create_signed_tx(
            eth_op_idx,
            &eth_sender,
            execute_operation.clone(),
            deadline_block,
            nonce,
        )
        .await;

        expected_txs.push(execute_op_tx);
    }

    let mut operation_iterator = commit_operations
        .iter()
        .zip(verify_operations)
        .zip(execute_operations);

    // Then we go through the operations and check that the order of operations is preserved.
    // Here we take N txs at each interaction.
    for txs in expected_txs.chunks(MAX_TXS_IN_FLIGHT as usize) {
        // We send operations by three, so the order will be "commit-verify-execute".
        // If we'll send all the operations together, the order will be "commit-verify-execute".
        let ((commit_op, verify_op), execute_op) = operation_iterator.next().unwrap();

        eth_sender
            .db
            .send_aggregated_operation(commit_op.clone())
            .await
            .unwrap();
        eth_sender
            .db
            .send_aggregated_operation(verify_op.clone())
            .await
            .unwrap();
        eth_sender
            .db
            .send_aggregated_operation(execute_op.clone())
            .await
            .unwrap();

        eth_sender.load_new_operations().await.unwrap();

        // Call `proceed_next_operations`. Several txs should be sent.
        eth_sender.proceed_next_operations().await;

        let commit_tx = &txs[0];
        let verify_tx = &txs[1];
        let execute_tx = &txs[2];

        // Check that commit/verify txs are sent and add the successful execution for them.
        for tx in &[commit_tx, verify_tx, execute_tx] {
            let current_tx_hash = tx.used_tx_hashes[0];

            // Check that current expected tx is stored.
            eth_sender.db.assert_stored(&tx).await;
            eth_sender
                .ethereum
                .get_mock()
                .unwrap()
                .assert_sent(&current_tx_hash.as_bytes().to_vec())
                .await;

            // Mark the tx as successfully
            eth_sender
                .ethereum
                .get_mut_mock()
                .unwrap()
                .add_successfull_execution(current_tx_hash, WAIT_CONFIRMATIONS)
                .await;
        }

        // Call `proceed_next_operations` again. Both txs should become confirmed.
        eth_sender.proceed_next_operations().await;

        for &tx in &[commit_tx, verify_tx, execute_tx] {
            let mut tx = tx.clone();
            let current_tx_hash = tx.used_tx_hashes[0];

            // Update the fields in the tx and check if it's confirmed.
            tx.confirmed = true;
            tx.final_hash = Some(current_tx_hash);
            eth_sender.db.assert_confirmed(&tx).await;
        }
    }
}
