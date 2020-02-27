use super::ETHSender;

use self::mock::default_eth_sender;

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

/// Test for a normal `ETHSender` workflow: we send the two sequential
/// operations (commit and verify), they are successfully committed to
/// the Ethereum, and notification is sent after `verify` operation
/// is committed.
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
    for idx in 0..operations.len() {
        // We start from the 1 block, and step logic is:
        // N blocks to confirm, repeated `idx` times.
        let start_block = 1 + super::WAIT_CONFIRMATIONS * idx as u64;
        let expected_tx = eth_sender
            .create_new_tx(
                &operations[idx],
                eth_sender.get_deadline_block(start_block),
                None,
            )
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
    for idx in 0..operations.len() {
        eth_sender.proceed_next_operation();

        // Check that current expected tx is stored, but the next ones are not.
        eth_sender.db.assert_stored(&expected_txs[idx]);
        eth_sender.ethereum.assert_sent(&expected_txs[idx]);

        for following_idx in (idx + 1)..operations.len() {
            eth_sender
                .db
                .assert_not_stored(&expected_txs[following_idx])
        }

        eth_sender
            .ethereum
            .add_successfull_execution(&expected_txs[idx], super::WAIT_CONFIRMATIONS);
        eth_sender.proceed_next_operation();
        eth_sender.db.assert_confirmed(&expected_txs[idx]);
    }

    // We should be notified about all the verify operations being completed.
    for _ in 0..operations_count {
        assert!(receiver.try_next().unwrap().is_some());
    }
}
