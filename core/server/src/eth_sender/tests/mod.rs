// Local uses
use self::mock::{default_eth_sender, restored_eth_sender};
use super::{
    ethereum_interface::EthereumInterface,
    transactions::{ExecutedTxStatus, OperationETHState, TransactionETHState, TxCheckOutcome},
    ETHSender,
};

mod mock;
mod test_data;

/// Basic test that `ETHSender` creation does not panic and initializes correctly.
#[test]
fn basic_test() {
    let (eth_sender, _, _) = default_eth_sender();

    // Check that there are no unconfirmed operations by default.
    assert!(eth_sender.unconfirmed_ops.is_empty());
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

/// Checks that `ETHSender` invokes `EthereumInterface::sign_call_tx` to obtain the
/// raw transaction contents (and does not mess with it).
#[test]
fn tx_creation() {
    let (eth_sender, _, _) = default_eth_sender();

    let operations = vec![
        test_data::commit_operation(0),
        test_data::verify_operation(0),
    ];

    for operation in operations {
        let actual_tx = eth_sender
            .create_new_tx(
                &operation,
                eth_sender.get_deadline_block(eth_sender.ethereum.block_number),
                None,
            )
            .unwrap();
        let expected_tx = eth_sender.ethereum.create_signed_tx_replica(&operation);
        assert_eq!(actual_tx.signed_tx, expected_tx);
    }
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
    let operations: Vec<TransactionETHState> = vec![
        test_data::commit_operation(0), // Will be committed.
        test_data::commit_operation(1), // Will be pending because of not enough confirmations.
        test_data::commit_operation(2), // Will be failed.
        test_data::commit_operation(3), // Will be stuck.
        test_data::commit_operation(4), // Will be pending due no response.
    ]
    .iter()
    .map(|op| eth_sender.create_new_tx(op, deadline_block, None).unwrap())
    .collect();

    // Committed operation.
    let committed_response = ExecutedTxStatus {
        confirmations: super::WAIT_CONFIRMATIONS,
        success: true,
        receipt: None,
    };
    eth_sender
        .ethereum
        .add_execution(&operations[0], &committed_response);

    // Pending operation.
    let pending_response = ExecutedTxStatus {
        confirmations: super::WAIT_CONFIRMATIONS - 1,
        success: true,
        receipt: None,
    };
    eth_sender
        .ethereum
        .add_execution(&operations[1], &pending_response);

    // Failed operation.
    let failed_response = ExecutedTxStatus {
        confirmations: super::WAIT_CONFIRMATIONS,
        success: false,
        receipt: Some(Default::default()),
    };
    eth_sender
        .ethereum
        .add_execution(&operations[2], &failed_response);

    // Checks.

    // Committed operation.
    assert_eq!(
        eth_sender
            .check_transaction_state(
                &operations[0],
                current_block + committed_response.confirmations
            )
            .unwrap(),
        TxCheckOutcome::Committed
    );

    // Pending operation (no enough confirmations).
    assert_eq!(
        eth_sender
            .check_transaction_state(
                &operations[1],
                current_block + pending_response.confirmations
            )
            .unwrap(),
        TxCheckOutcome::Pending
    );

    // Failed operation.
    assert_eq!(
        eth_sender
            .check_transaction_state(
                &operations[2],
                current_block + failed_response.confirmations
            )
            .unwrap(),
        TxCheckOutcome::Failed(Default::default())
    );

    // Stuck operation.
    assert_eq!(
        eth_sender
            .check_transaction_state(
                &operations[3],
                current_block + super::EXPECTED_WAIT_TIME_BLOCKS
            )
            .unwrap(),
        TxCheckOutcome::Stuck
    );

    // Pending operation (no response yet).
    assert_eq!(
        eth_sender
            .check_transaction_state(
                &operations[4],
                current_block + super::EXPECTED_WAIT_TIME_BLOCKS - 1
            )
            .unwrap(),
        TxCheckOutcome::Pending
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

    for operation in operations {
        // Send an operation to `ETHSender`.
        sender.try_send(operation.clone()).unwrap();

        // Retrieve it there and then process.
        eth_sender.retrieve_operations();
        eth_sender.proceed_next_operation();

        // Now we should see that transaction is stored in the database and sent to the Ethereum.
        let expected_tx = eth_sender
            .create_new_tx(
                &operation,
                eth_sender.get_deadline_block(eth_sender.ethereum.block_number),
                None,
            )
            .unwrap();
        eth_sender.db.assert_stored(&expected_tx);
        eth_sender.ethereum.assert_sent(&expected_tx);

        // No confirmation should be done yet.
        assert!(receiver.try_next().is_err());

        // Increment block, make the transaction look successfully executed, and process the
        // operation again.
        eth_sender
            .ethereum
            .add_successfull_execution(&expected_tx, super::WAIT_CONFIRMATIONS);
        eth_sender.proceed_next_operation();

        // Check that operation is confirmed.
        eth_sender.db.assert_confirmed(&expected_tx);
    }

    // Check that `completeWithdrawals` transaction is sent.
    let tx = eth_sender
        .ethereum
        .sign_call_tx(
            "completeWithdrawals",
            models::node::config::MAX_WITHDRAWALS_TO_COMPLETE_IN_A_CALL,
            Default::default(),
        )
        .unwrap();
    eth_sender.ethereum.assert_sent_by_hash(&tx.hash);

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
    eth_sender.proceed_next_operation();

    let stuck_tx = eth_sender
        .create_new_tx(
            &operation,
            eth_sender.get_deadline_block(eth_sender.ethereum.block_number),
            None,
        )
        .unwrap();

    // Skip some blocks and expect sender to send a new tx.
    eth_sender.ethereum.block_number += super::EXPECTED_WAIT_TIME_BLOCKS;
    eth_sender.ethereum.nonce += 1.into();
    eth_sender.proceed_next_operation();

    // Check that new transaction is sent (and created based on the previous stuck tx).
    let expected_tx = eth_sender
        .create_new_tx(
            &operation,
            eth_sender.get_deadline_block(eth_sender.ethereum.block_number),
            Some(&stuck_tx),
        )
        .unwrap();
    eth_sender.db.assert_stored(&expected_tx);
    eth_sender.ethereum.assert_sent(&expected_tx);

    // Increment block, make the transaction look successfully executed, and process the
    // operation again.
    eth_sender
        .ethereum
        .add_successfull_execution(&expected_tx, super::WAIT_CONFIRMATIONS);
    eth_sender.proceed_next_operation();

    // Check that operation is confirmed.
    eth_sender.db.assert_confirmed(&expected_tx);
}

/// This test verifies that with multiple operations received all-together,
/// their order is respected and no processing of the next operation is started until
/// the previous one is committed.
#[test]
fn operations_order() {
    let (mut eth_sender, mut sender, mut receiver) = default_eth_sender();

    // We send multiple the operations at once to the channel.
    let operations_count = 3;
    let mut operations = Vec::new();
    operations.extend_from_slice(&test_data::COMMIT_OPERATIONS[..operations_count]);
    operations.extend_from_slice(&test_data::VERIFY_OPERATIONS[..operations_count]);

    // Also we create the list of expected transactions.
    let mut expected_txs = Vec::new();
    for (idx, operation) in operations.iter().enumerate() {
        // We start from the 1 block, and step logic is:
        // N blocks to confirm, repeated `idx` times.
        let start_block = 1 + super::WAIT_CONFIRMATIONS * idx as u64;
        let expected_tx = eth_sender
            .create_new_tx(operation, eth_sender.get_deadline_block(start_block), None)
            .unwrap();

        // Update nonce as well (it will be reset below).
        eth_sender.ethereum.nonce += 1.into();

        expected_txs.push(expected_tx);
    }

    // Reset nonce (it was affected by creating expected transactions).
    eth_sender.ethereum.nonce = 0.into();

    for operation in operations.iter() {
        sender.try_send(operation.clone()).unwrap();
    }
    eth_sender.retrieve_operations();

    // Then we go through the operations and check that the order of operations is preserved.
    for (idx, tx) in expected_txs.iter().enumerate() {
        eth_sender.proceed_next_operation();

        // Check that current expected tx is stored, but the next ones are not.
        eth_sender.db.assert_stored(tx);
        eth_sender.ethereum.assert_sent(tx);

        for following_tx in expected_txs[idx + 1..].iter() {
            eth_sender.db.assert_not_stored(following_tx)
        }

        eth_sender
            .ethereum
            .add_successfull_execution(tx, super::WAIT_CONFIRMATIONS);
        eth_sender.proceed_next_operation();
        eth_sender.db.assert_confirmed(tx);
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

    let failing_tx = eth_sender
        .create_new_tx(
            &operation,
            eth_sender.get_deadline_block(eth_sender.ethereum.block_number),
            None,
        )
        .unwrap();

    eth_sender.retrieve_operations();
    eth_sender.proceed_next_operation();

    eth_sender
        .ethereum
        .add_failed_execution(&failing_tx, super::WAIT_CONFIRMATIONS);
    eth_sender.proceed_next_operation();
}

/// Check that after recovering state with several non-processed operations
/// they will be processed normally.
#[test]
fn restore_state() {
    let operations = vec![
        test_data::commit_operation(0),
        test_data::verify_operation(0),
    ];

    // Create `OperationETHState` objects from operations and restore state.
    let stored_operations = operations.iter().map(|operation| OperationETHState {
        operation: operation.clone(),
        txs: Vec::new(),
    });

    let (mut eth_sender, _, mut receiver) = restored_eth_sender(stored_operations);

    for operation in operations {
        // Note that we DO NOT send an operation to `ETHSender` and neither receive it.

        // We do process operations restored from the DB though.
        // The rest of this test is the same as in `operation_commitment_workflow`.
        eth_sender.proceed_next_operation();

        let expected_tx = eth_sender
            .create_new_tx(
                &operation,
                eth_sender.get_deadline_block(eth_sender.ethereum.block_number),
                None,
            )
            .unwrap();

        eth_sender
            .ethereum
            .add_successfull_execution(&expected_tx, super::WAIT_CONFIRMATIONS);
        eth_sender.proceed_next_operation();
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
    eth_sender.proceed_next_operation();

    let stuck_tx = eth_sender
        .create_new_tx(
            &operation,
            eth_sender.get_deadline_block(eth_sender.ethereum.block_number),
            None,
        )
        .unwrap();

    eth_sender.ethereum.block_number += super::EXPECTED_WAIT_TIME_BLOCKS;
    eth_sender.ethereum.nonce += 1.into();
    eth_sender.proceed_next_operation();

    let next_tx = eth_sender
        .create_new_tx(
            &operation,
            eth_sender.get_deadline_block(eth_sender.ethereum.block_number),
            Some(&stuck_tx),
        )
        .unwrap();
    eth_sender.db.assert_stored(&next_tx);
    eth_sender.ethereum.assert_sent(&next_tx);

    // Add a confirmation for a *stuck* transaction.
    eth_sender
        .ethereum
        .add_successfull_execution(&stuck_tx, super::WAIT_CONFIRMATIONS);
    eth_sender.proceed_next_operation();

    // Check that operation is confirmed.
    eth_sender.db.assert_confirmed(&stuck_tx);
}
