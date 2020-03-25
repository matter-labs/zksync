// External uses
// Workspace uses
use models::ethereum::ETHOperation;
// Local uses
use self::mock::{
    create_signed_tx, create_signed_withdraw_tx, default_eth_sender, restored_eth_sender,
};
use super::{
    transactions::{ETHStats, ExecutedTxStatus, TxCheckOutcome},
    ETHSender, TxCheckMode,
};

mod mock;
mod test_data;

/// Basic test that `ETHSender` creation does not panic and initializes correctly.
#[test]
fn basic_test() {
    let (eth_sender, _, _) = default_eth_sender();

    // Check that there are no unconfirmed operations by default.
    assert!(eth_sender.ongoing_ops.is_empty());
}

/// Check for the gas scaling: gas is expected to be increased by 15% or set equal
/// to gas cost suggested by Ethereum (if it's greater).
#[test]
fn scale_gas() {
    let (mut eth_sender, _, _) = default_eth_sender();

    // Set the gas price in Ethereum to 1000.
    eth_sender.ethereum.gas_price = 1000.into();

    // Check that gas price of 1000 is increased to 1150.
    let scaled_gas = eth_sender.scale_gas(1000.into()).unwrap();
    assert_eq!(scaled_gas, 1150.into());

    // Check that gas price of 100 is increased to 1000 (price in Ethereum object).
    let scaled_gas = eth_sender.scale_gas(100.into()).unwrap();
    assert_eq!(scaled_gas, 1000.into());
}

/// Checks that deadline block is chosen according to the expected policy.
#[test]
fn deadline_block() {
    let (eth_sender, _, _) = default_eth_sender();

    assert_eq!(
        eth_sender.get_deadline_block(0),
        super::EXPECTED_WAIT_TIME_BLOCKS
    );
    assert_eq!(
        eth_sender.get_deadline_block(10),
        10 + super::EXPECTED_WAIT_TIME_BLOCKS
    );
}

/// Checks that received transaction response is reduced to the
/// `TxCheckOutcome` correctly.
///
/// Here we check every possible output of the `check_transaction_state` method.
#[test]
fn transaction_state() {
    let (mut eth_sender, _, _) = default_eth_sender();
    let current_block = eth_sender.ethereum.block_number;
    let deadline_block = eth_sender.get_deadline_block(current_block);
    let operations: Vec<ETHOperation> = vec![
        test_data::commit_operation(0), // Will be committed.
        test_data::commit_operation(1), // Will be pending because of not enough confirmations.
        test_data::commit_operation(2), // Will be failed.
        test_data::commit_operation(3), // Will be stuck.
        test_data::commit_operation(4), // Will be pending due no response.
    ]
    .iter()
    .enumerate()
    .map(|(eth_op_id, op)| {
        let nonce = eth_op_id as i64;
        create_signed_tx(eth_op_id as i64, &eth_sender, op, deadline_block, nonce)
    })
    .collect();

    // Committed operation.
    let committed_response = ExecutedTxStatus {
        confirmations: super::WAIT_CONFIRMATIONS,
        success: true,
        receipt: None,
    };
    eth_sender
        .ethereum
        .add_execution(&operations[0].used_tx_hashes[0], &committed_response);

    // Pending operation.
    let pending_response = ExecutedTxStatus {
        confirmations: super::WAIT_CONFIRMATIONS - 1,
        success: true,
        receipt: None,
    };
    eth_sender
        .ethereum
        .add_execution(&operations[1].used_tx_hashes[0], &pending_response);

    // Failed operation.
    let failed_response = ExecutedTxStatus {
        confirmations: super::WAIT_CONFIRMATIONS,
        success: false,
        receipt: Some(Default::default()),
    };
    eth_sender
        .ethereum
        .add_execution(&operations[2].used_tx_hashes[0], &failed_response);

    // Checks.

    // Committed operation.
    assert_eq!(
        eth_sender
            .check_transaction_state(
                TxCheckMode::Latest,
                &operations[0],
                &operations[0].used_tx_hashes[0],
                current_block + committed_response.confirmations
            )
            .unwrap(),
        TxCheckOutcome::Committed
    );

    // Pending operation (no enough confirmations).
    assert_eq!(
        eth_sender
            .check_transaction_state(
                TxCheckMode::Latest,
                &operations[1],
                &operations[1].used_tx_hashes[0],
                current_block + pending_response.confirmations
            )
            .unwrap(),
        TxCheckOutcome::Pending
    );

    // Failed operation.
    assert_eq!(
        eth_sender
            .check_transaction_state(
                TxCheckMode::Latest,
                &operations[2],
                &operations[2].used_tx_hashes[0],
                current_block + failed_response.confirmations
            )
            .unwrap(),
        TxCheckOutcome::Failed(Default::default())
    );

    // Stuck operation.
    assert_eq!(
        eth_sender
            .check_transaction_state(
                TxCheckMode::Latest,
                &operations[3],
                &operations[3].used_tx_hashes[0],
                current_block + super::EXPECTED_WAIT_TIME_BLOCKS
            )
            .unwrap(),
        TxCheckOutcome::Stuck
    );

    // Pending operation (no response yet).
    assert_eq!(
        eth_sender
            .check_transaction_state(
                TxCheckMode::Latest,
                &operations[4],
                &operations[4].used_tx_hashes[0],
                current_block + super::EXPECTED_WAIT_TIME_BLOCKS - 1
            )
            .unwrap(),
        TxCheckOutcome::Pending
    );

    // Pending old operation should be considered stuck.
    assert_eq!(
        eth_sender
            .check_transaction_state(
                TxCheckMode::Old,
                &operations[4],
                &operations[4].used_tx_hashes[0],
                current_block + super::EXPECTED_WAIT_TIME_BLOCKS - 1
            )
            .unwrap(),
        TxCheckOutcome::Stuck
    );
}

/// Test for a normal `ETHSender` workflow:
/// - we send the two sequential operations (commit and verify);
/// - they are successfully committed to the Ethereum;
/// - `completeWithdrawals` tx is sent to the Ethereum;
/// - notification is sent after `verify` operation is committed.
#[test]
fn operation_commitment_workflow() {
    let (mut eth_sender, mut sender, mut receiver) = default_eth_sender();

    // In this test we will run one commit and one verify operation and should
    // obtain a notification about the operation being completed in the end.
    let operations = vec![
        test_data::commit_operation(0),
        test_data::verify_operation(0),
    ];

    let verify_operation_id = operations[1].id;

    for (eth_op_id, operation) in operations.iter().enumerate() {
        let nonce = eth_op_id as i64;

        // Send an operation to `ETHSender`.
        sender.try_send(operation.clone()).unwrap();

        // Retrieve it there and then process.
        eth_sender.retrieve_operations();
        eth_sender.proceed_next_operations();

        // Now we should see that transaction is stored in the database and sent to the Ethereum.
        let deadline_block = eth_sender.get_deadline_block(eth_sender.ethereum.block_number);
        let mut expected_tx = create_signed_tx(
            eth_op_id as i64,
            &eth_sender,
            operation,
            deadline_block,
            nonce,
        );
        expected_tx.id = eth_op_id as i64; // We have to set the ID manually.

        eth_sender.db.assert_stored(&expected_tx);
        eth_sender
            .ethereum
            .assert_sent(&expected_tx.used_tx_hashes[0]);

        // No confirmation should be done yet.
        assert!(receiver.try_next().is_err());

        // Increment block, make the transaction look successfully executed, and process the
        // operation again.
        eth_sender
            .ethereum
            .add_successfull_execution(expected_tx.used_tx_hashes[0], super::WAIT_CONFIRMATIONS);
        eth_sender.proceed_next_operations();

        // Check that operation is confirmed.
        expected_tx.confirmed = true;
        expected_tx.final_hash = Some(expected_tx.used_tx_hashes[0]);
        eth_sender.db.assert_confirmed(&expected_tx);
    }

    // Process the next operation and check that `completeWithdrawals` transaction is stored and sent.
    eth_sender.proceed_next_operations();

    let eth_op_idx = operations.len() as i64;
    let nonce = eth_op_idx;
    let deadline_block = eth_sender.get_deadline_block(eth_sender.ethereum.block_number);
    let mut withdraw_op_tx =
        create_signed_withdraw_tx(eth_op_idx, &eth_sender, deadline_block, nonce);

    eth_sender.db.assert_stored(&withdraw_op_tx);
    eth_sender
        .ethereum
        .assert_sent(&withdraw_op_tx.used_tx_hashes[0]);

    // Mark `completeWithdrawals` as completed.
    eth_sender
        .ethereum
        .add_successfull_execution(withdraw_op_tx.used_tx_hashes[0], super::WAIT_CONFIRMATIONS);
    eth_sender.proceed_next_operations();

    // Check that `completeWithdrawals` is completed in the DB.
    withdraw_op_tx.confirmed = true;
    withdraw_op_tx.final_hash = Some(withdraw_op_tx.used_tx_hashes[0]);
    eth_sender.db.assert_confirmed(&withdraw_op_tx);

    // We should be notified about verify operation being completed.
    assert_eq!(
        receiver.try_next().unwrap().unwrap().id,
        verify_operation_id
    );
}

/// A simple scenario for a stuck transaction:
/// - A transaction is sent to the Ethereum.
/// - It is not processed after some blocks.
/// - `ETHSender` creates a new transaction with increased gas.
/// - This transaction is completed successfully.
#[test]
fn stuck_transaction() {
    let (mut eth_sender, mut sender, _) = default_eth_sender();

    // Workflow for the test is similar to `operation_commitment_workflow`.
    let operation = test_data::commit_operation(0);
    sender.try_send(operation.clone()).unwrap();

    eth_sender.retrieve_operations();
    eth_sender.proceed_next_operations();

    let eth_op_id = 0;
    let nonce = 0;
    let deadline_block = eth_sender.get_deadline_block(eth_sender.ethereum.block_number);
    let mut stuck_tx = create_signed_tx(eth_op_id, &eth_sender, &operation, deadline_block, nonce);

    // Skip some blocks and expect sender to send a new tx.
    eth_sender.ethereum.block_number += super::EXPECTED_WAIT_TIME_BLOCKS;
    eth_sender.proceed_next_operations();

    // Check that new transaction is sent (and created based on the previous stuck tx).
    let expected_sent_tx = eth_sender
        .create_supplement_tx(
            eth_sender.get_deadline_block(eth_sender.ethereum.block_number),
            &mut stuck_tx,
        )
        .unwrap();
    eth_sender.db.assert_stored(&stuck_tx);
    eth_sender.ethereum.assert_sent(&expected_sent_tx.hash);

    // Increment block, make the transaction look successfully executed, and process the
    // operation again.
    eth_sender
        .ethereum
        .add_successfull_execution(stuck_tx.used_tx_hashes[1], super::WAIT_CONFIRMATIONS);
    eth_sender.proceed_next_operations();

    // Check that operation is confirmed (we set the final hash to the second sent tx).
    stuck_tx.confirmed = true;
    stuck_tx.final_hash = Some(stuck_tx.used_tx_hashes[1]);
    eth_sender.db.assert_confirmed(&stuck_tx);
}

/// This test verifies that with multiple operations received all-together,
/// their order is respected and no processing of the next operation is started until
/// the previous one is committed.
///
/// This test includes all three operation types (commit, verify and withdraw).
#[test]
fn operations_order() {
    let (mut eth_sender, mut sender, mut receiver) = default_eth_sender();

    // We send multiple the operations at once to the channel.
    let operations_count = 3;
    let mut operations = Vec::new();
    let commit_operations = &test_data::COMMIT_OPERATIONS[..operations_count];
    let verify_operations = &test_data::VERIFY_OPERATIONS[..operations_count];
    operations.extend_from_slice(commit_operations);
    operations.extend_from_slice(verify_operations);

    // Also we create the list of expected transactions.
    let mut expected_txs = Vec::new();

    // Create expected txs from all the operations.
    // Since we create 3 operations at each cycle iteration,
    // the logic of ID calculating is (i * 3), (i * 3 + 1), (i * 3 + 2).
    // On the first iteration the indices 0, 1 and 2 will be taken, then it
    // will be 3, 4 and 5, etc.
    for (idx, (commit_operation, verify_operation)) in
        commit_operations.iter().zip(verify_operations).enumerate()
    {
        // Create the commit operation.
        let start_block = 1 + super::WAIT_CONFIRMATIONS * (idx * 3) as u64;
        let deadline_block = eth_sender.get_deadline_block(start_block);
        let eth_op_idx = (idx * 3) as i64;
        let nonce = eth_op_idx;

        let commit_op_tx = create_signed_tx(
            eth_op_idx,
            &eth_sender,
            commit_operation,
            deadline_block,
            nonce,
        );

        expected_txs.push(commit_op_tx);

        // Create the verify operation, as by priority it will be processed right after `commit`.
        let start_block = 1 + super::WAIT_CONFIRMATIONS * (idx * 3 + 1) as u64;
        let deadline_block = eth_sender.get_deadline_block(start_block);
        let eth_op_idx = (idx * 3 + 1) as i64;
        let nonce = eth_op_idx;

        let verify_op_tx = create_signed_tx(
            eth_op_idx,
            &eth_sender,
            verify_operation,
            deadline_block,
            nonce,
        );

        expected_txs.push(verify_op_tx);

        // Create the withdraw operation.
        let start_block = 1 + super::WAIT_CONFIRMATIONS * (idx * 3 + 2) as u64;
        let deadline_block = eth_sender.get_deadline_block(start_block);
        let eth_op_idx = (idx * 3 + 2) as i64;
        let nonce = eth_op_idx;

        let withdraw_op_tx =
            create_signed_withdraw_tx(eth_op_idx, &eth_sender, deadline_block, nonce);

        expected_txs.push(withdraw_op_tx);
    }

    for operation in operations.iter() {
        sender.try_send(operation.clone()).unwrap();
    }
    eth_sender.retrieve_operations();

    // Then we go through the operations and check that the order of operations is preserved.
    for mut tx in expected_txs.into_iter() {
        let current_tx_hash = tx.used_tx_hashes[0];

        eth_sender.proceed_next_operations();

        // Check that current expected tx is stored.
        eth_sender.db.assert_stored(&tx);
        eth_sender.ethereum.assert_sent(&current_tx_hash);

        // Mark the tx as successfully
        eth_sender
            .ethereum
            .add_successfull_execution(current_tx_hash, super::WAIT_CONFIRMATIONS);
        eth_sender.proceed_next_operations();

        // Update the fields in the tx and check if it's confirmed.
        tx.confirmed = true;
        tx.final_hash = Some(current_tx_hash);
        eth_sender.db.assert_confirmed(&tx);
    }

    // We should be notified about all the verify operations being completed.
    for _ in 0..operations_count {
        assert!(receiver.try_next().unwrap().is_some());
    }
}

/// Check that upon a transaction failure the incident causes a panic by default.
#[test]
#[should_panic(expected = "Cannot operate after unexpected TX failure")]
fn transaction_failure() {
    let (mut eth_sender, mut sender, _) = default_eth_sender();

    // Workflow for the test is similar to `operation_commitment_workflow`.
    let operation = test_data::commit_operation(0);
    sender.try_send(operation.clone()).unwrap();

    let eth_op_id = 0;
    let nonce = 0;
    let deadline_block = eth_sender.get_deadline_block(eth_sender.ethereum.block_number);
    let failing_tx = create_signed_tx(eth_op_id, &eth_sender, &operation, deadline_block, nonce);

    eth_sender.retrieve_operations();
    eth_sender.proceed_next_operations();

    eth_sender
        .ethereum
        .add_failed_execution(&failing_tx.used_tx_hashes[0], super::WAIT_CONFIRMATIONS);
    eth_sender.proceed_next_operations();
}

/// Check that after recovering state with several non-processed operations
/// they will be processed normally.
#[test]
fn restore_state() {
    let (operations, stored_operations) = {
        // This `eth_sender` is required to generate the input only.
        let (eth_sender, _, _) = default_eth_sender();

        let commit_op = test_data::commit_operation(0);
        let verify_op = test_data::verify_operation(0);

        let deadline_block = eth_sender.get_deadline_block(1);
        let commit_op_tx = create_signed_tx(0, &eth_sender, &commit_op, deadline_block, 0);

        let deadline_block = eth_sender.get_deadline_block(2);
        let verify_op_tx = create_signed_tx(1, &eth_sender, &verify_op, deadline_block, 1);

        let operations = vec![commit_op.clone(), verify_op.clone()];
        let stored_operations = vec![commit_op_tx, verify_op_tx];

        (operations, stored_operations)
    };

    let stats = ETHStats {
        commit_ops: 1,
        verify_ops: 1,
        withdraw_ops: 0,
    };
    let (mut eth_sender, _, mut receiver) = restored_eth_sender(stored_operations.clone(), stats);

    for (eth_op_id, operation) in operations.iter().enumerate() {
        // Note that we DO NOT send an operation to `ETHSender` and neither receive it.

        // We do process operations restored from the DB though.
        // The rest of this test is the same as in `operation_commitment_workflow`.
        eth_sender.proceed_next_operations();

        let deadline_block = eth_sender.get_deadline_block(eth_sender.ethereum.block_number);
        let nonce = eth_op_id as i64;
        let mut expected_tx = create_signed_tx(
            eth_op_id as i64,
            &eth_sender,
            operation,
            deadline_block,
            nonce,
        );
        expected_tx.id = eth_op_id as i64;

        eth_sender.db.assert_stored(&expected_tx);

        eth_sender
            .ethereum
            .add_successfull_execution(expected_tx.used_tx_hashes[0], super::WAIT_CONFIRMATIONS);
        eth_sender.proceed_next_operations();

        expected_tx.confirmed = true;
        expected_tx.final_hash = Some(expected_tx.used_tx_hashes[0]);
        eth_sender.db.assert_confirmed(&expected_tx);
    }

    assert!(receiver.try_next().unwrap().is_some());
}

/// Checks that even after getting the first transaction stuck and sending the next
/// one, confirmation for the first (stuck) transaction is processed and leads
/// to the operation commitment.
#[test]
fn confirmations_independence() {
    // Workflow in the test is the same as in `stuck_transaction`, except for the fact
    // that confirmation is obtained for the stuck transaction instead of the latter one.

    let (mut eth_sender, mut sender, _) = default_eth_sender();

    let operation = test_data::commit_operation(0);
    sender.try_send(operation.clone()).unwrap();

    eth_sender.retrieve_operations();
    eth_sender.proceed_next_operations();

    let eth_op_id = 0;
    let nonce = 0;
    let deadline_block = eth_sender.get_deadline_block(eth_sender.ethereum.block_number);
    let mut stuck_tx = create_signed_tx(eth_op_id, &eth_sender, &operation, deadline_block, nonce);

    eth_sender.ethereum.block_number += super::EXPECTED_WAIT_TIME_BLOCKS;
    eth_sender.proceed_next_operations();

    let next_tx = eth_sender
        .create_supplement_tx(
            eth_sender.get_deadline_block(eth_sender.ethereum.block_number),
            &mut stuck_tx,
        )
        .unwrap();
    eth_sender.db.assert_stored(&stuck_tx);
    eth_sender.ethereum.assert_sent(&next_tx.hash);

    // Add a confirmation for a *stuck* transaction.
    eth_sender
        .ethereum
        .add_successfull_execution(stuck_tx.used_tx_hashes[0], super::WAIT_CONFIRMATIONS);
    eth_sender.proceed_next_operations();

    // Check that operation is confirmed (we set the final hash to the *first* sent tx).
    stuck_tx.confirmed = true;
    stuck_tx.final_hash = Some(stuck_tx.used_tx_hashes[0]);
    eth_sender.db.assert_confirmed(&stuck_tx);
}
