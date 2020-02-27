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
