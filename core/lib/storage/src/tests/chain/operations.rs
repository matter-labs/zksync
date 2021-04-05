// External imports
use chrono::{Duration, Utc};
// Workspace imports
use zksync_types::{aggregated_operations::AggregatedActionType, BlockNumber};
// Local imports
use crate::{
    chain::{
        block::BlockSchema,
        operations::{
            records::{NewExecutedPriorityOperation, NewExecutedTransaction},
            OperationsSchema,
        },
    },
    test_data::gen_unique_aggregated_operation,
    tests::db_test,
    QueryResult, StorageProcessor,
};

/// Checks the save&load routine for unconfirmed operations.
#[db_test]
async fn aggregated_operations(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let block_number = 1;
    let action_type = AggregatedActionType::CommitBlocks;
    OperationsSchema(&mut storage)
        .store_aggregated_action(gen_unique_aggregated_operation(
            BlockNumber(block_number),
            action_type,
            100,
        ))
        .await?;

    let stored_operation = OperationsSchema(&mut storage)
        .get_stored_aggregated_operation(BlockNumber(block_number as u32), action_type)
        .await
        .unwrap();

    assert_eq!(stored_operation.from_block, 1);
    assert_eq!(stored_operation.to_block, 1);
    assert_eq!(stored_operation.action_type, action_type.to_string());
    assert_eq!(stored_operation.confirmed, false);

    Ok(())
}

/// Checks the save&load routine for executed operations.
#[db_test]
async fn executed_operations(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let executed_tx = NewExecutedTransaction {
        block_number: 1,
        tx_hash: vec![0xDE, 0xAD, 0xBE, 0xEF],
        tx: Default::default(),
        operation: Default::default(),
        from_account: Default::default(),
        to_account: None,
        success: true,
        fail_reason: None,
        block_index: None,
        primary_account_address: Default::default(),
        nonce: Default::default(),
        created_at: chrono::Utc::now(),
        eth_sign_data: None,
        batch_id: Some(10),
    };

    OperationsSchema(&mut storage)
        .store_executed_tx(executed_tx.clone())
        .await?;

    let stored_operation = OperationsSchema(&mut storage)
        .get_executed_operation(executed_tx.tx_hash.as_ref())
        .await?
        .expect("No operation was found for a valid hash");

    assert_eq!(stored_operation.block_number, executed_tx.block_number);
    assert_eq!(stored_operation.tx_hash, executed_tx.tx_hash);
    assert_eq!(stored_operation.tx, executed_tx.tx);
    assert_eq!(stored_operation.operation, executed_tx.operation);
    assert_eq!(stored_operation.from_account, executed_tx.from_account);
    assert_eq!(stored_operation.to_account, executed_tx.to_account);
    assert_eq!(stored_operation.success, executed_tx.success);
    assert_eq!(stored_operation.fail_reason, executed_tx.fail_reason);
    assert_eq!(stored_operation.block_index, executed_tx.block_index);
    assert_eq!(stored_operation.nonce, executed_tx.nonce);
    assert_eq!(
        stored_operation.primary_account_address,
        executed_tx.primary_account_address
    );
    assert_eq!(stored_operation.batch_id, executed_tx.batch_id);

    Ok(())
}

/// Checks the save&load routine for executed priority operations.
#[db_test]
async fn executed_priority_operations(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let executed_tx = NewExecutedPriorityOperation {
        block_number: 1,
        block_index: 1,
        operation: Default::default(),
        from_account: Default::default(),
        to_account: Default::default(),
        priority_op_serialid: 0,
        deadline_block: 100,
        eth_hash: vec![0xDE, 0xAD, 0xBE, 0xEF],
        eth_block: 10,
        created_at: chrono::Utc::now(),
    };
    OperationsSchema(&mut storage)
        .store_executed_priority_op(executed_tx.clone())
        .await?;

    let stored_operation = OperationsSchema(&mut storage)
        .get_executed_priority_operation(executed_tx.priority_op_serialid as u32)
        .await?
        .expect("No operation was found for a valid hash");

    assert_eq!(stored_operation.block_number, executed_tx.block_number);
    assert_eq!(stored_operation.block_index, executed_tx.block_index);
    assert_eq!(stored_operation.operation, executed_tx.operation);
    assert_eq!(stored_operation.from_account, executed_tx.from_account);
    assert_eq!(stored_operation.to_account, executed_tx.to_account);
    assert_eq!(
        stored_operation.priority_op_serialid,
        executed_tx.priority_op_serialid
    );
    assert_eq!(stored_operation.deadline_block, executed_tx.deadline_block);
    assert_eq!(stored_operation.eth_hash, executed_tx.eth_hash);

    Ok(())
}

/// Checks that attempt to save the duplicate txs is ignored by the DB.
#[db_test]
async fn duplicated_operations(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    const BLOCK_NUMBER: i64 = 1;

    let executed_tx = NewExecutedTransaction {
        block_number: BLOCK_NUMBER,
        tx_hash: vec![0x12, 0xAD, 0xBE, 0xEF],
        tx: Default::default(),
        operation: Default::default(),
        from_account: Default::default(),
        to_account: None,
        success: true,
        fail_reason: None,
        block_index: None,
        primary_account_address: Default::default(),
        nonce: Default::default(),
        created_at: chrono::Utc::now(),
        eth_sign_data: None,
        batch_id: None,
    };

    let executed_priority_op = NewExecutedPriorityOperation {
        block_number: BLOCK_NUMBER,
        block_index: 1,
        operation: Default::default(),
        from_account: Default::default(),
        to_account: Default::default(),
        priority_op_serialid: 0,
        deadline_block: 100,
        eth_hash: vec![0xDE, 0xAD, 0xBE, 0xEF],
        eth_block: 10,
        created_at: chrono::Utc::now(),
    };

    // Save the same operations twice.
    OperationsSchema(&mut storage)
        .store_executed_tx(executed_tx.clone())
        .await?;
    OperationsSchema(&mut storage)
        .store_executed_tx(executed_tx.clone())
        .await?;
    OperationsSchema(&mut storage)
        .store_executed_priority_op(executed_priority_op.clone())
        .await?;
    OperationsSchema(&mut storage)
        .store_executed_priority_op(executed_priority_op.clone())
        .await?;

    // Check that we can still load it.
    assert!(OperationsSchema(&mut storage)
        .get_executed_operation(executed_tx.tx_hash.as_ref())
        .await?
        .is_some());
    assert!(OperationsSchema(&mut storage)
        .get_executed_priority_operation(executed_priority_op.priority_op_serialid as u32)
        .await?
        .is_some());

    // Get the block transactions and check if there are exactly 2 txs.
    let block_txs = BlockSchema(&mut storage)
        .get_block_transactions(BlockNumber(BLOCK_NUMBER as u32))
        .await?;

    assert_eq!(block_txs.len(), 2);

    Ok(())
}

/// Checks that sending a successful operation after a failed one works.
#[db_test]
async fn transaction_resent(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    const BLOCK_NUMBER: i64 = 1;

    let mut executed_tx = NewExecutedTransaction {
        block_number: BLOCK_NUMBER,
        tx_hash: vec![0x12, 0xAD, 0xBE, 0xEF],
        tx: Default::default(),
        operation: Default::default(),
        from_account: Default::default(),
        to_account: None,
        success: false, // <- Note that success is false. We'll replace this tx with succeeded one.
        fail_reason: None,
        block_index: None,
        primary_account_address: Default::default(),
        nonce: Default::default(),
        created_at: chrono::Utc::now(),
        eth_sign_data: None,
        batch_id: None,
    };

    // Save the failed operation.
    OperationsSchema(&mut storage)
        .store_executed_tx(executed_tx.clone())
        .await?;

    // Check that we can still load it.
    assert!(OperationsSchema(&mut storage)
        .get_executed_operation(executed_tx.tx_hash.as_ref())
        .await?
        .is_some());

    // Replace failed tx with a successfull one.
    executed_tx.success = true;

    OperationsSchema(&mut storage)
        .store_executed_tx(executed_tx.clone())
        .await?;

    // Obtain tx and check that it's now successful.
    let loaded_tx = OperationsSchema(&mut storage)
        .get_executed_operation(executed_tx.tx_hash.as_ref())
        .await?
        .unwrap();
    assert_eq!(loaded_tx.tx_hash, executed_tx.tx_hash);
    assert_eq!(loaded_tx.success, true);

    // Get the block transactions and check if there is exactly 1 tx (failed tx not copied but replaced).
    let block_txs = BlockSchema(&mut storage)
        .get_block_transactions(BlockNumber(BLOCK_NUMBER as u32))
        .await?;
    assert_eq!(block_txs.len(), 1);

    // Now try to replace successfull transation wi`th a failed one.
    executed_tx.success = false;
    OperationsSchema(&mut storage)
        .store_executed_tx(executed_tx.clone())
        .await?;

    // ...it should not be replaced.
    let loaded_tx = OperationsSchema(&mut storage)
        .get_executed_operation(executed_tx.tx_hash.as_ref())
        .await?
        .unwrap();
    assert_eq!(loaded_tx.tx_hash, executed_tx.tx_hash);
    assert_eq!(loaded_tx.success, true);

    // ...and there still must be one operation.
    let block_txs = BlockSchema(&mut storage)
        .get_block_transactions(BlockNumber(BLOCK_NUMBER as u32))
        .await?;
    assert_eq!(block_txs.len(), 1);

    Ok(())
}

/// Checks that rejected transactions are removed correctly depending on the given age limit.
#[db_test]
async fn remove_rejected_transactions(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    const BLOCK_NUMBER: i64 = 1;
    // Two failed transactions created a week and two weeks ago respectively and one successful.
    let timestamp_1 = Utc::now() - Duration::weeks(1);
    let executed_tx_1 = NewExecutedTransaction {
        block_number: BLOCK_NUMBER,
        tx_hash: vec![0x12, 0xAD, 0xBE, 0xEF],
        tx: Default::default(),
        operation: Default::default(),
        from_account: Default::default(),
        to_account: None,
        success: false,
        fail_reason: None,
        block_index: None,
        primary_account_address: Default::default(),
        nonce: Default::default(),
        created_at: timestamp_1,
        eth_sign_data: None,
        batch_id: None,
    };
    let timestamp_2 = timestamp_1 - Duration::weeks(1);
    let mut executed_tx_2 = executed_tx_1.clone();
    // Set new timestamp and different tx_hash since it's a PK.
    executed_tx_2.created_at = timestamp_2;
    executed_tx_2.tx_hash = vec![0, 11, 21, 5];
    // Successful one.
    let mut executed_tx_3 = executed_tx_1.clone();
    executed_tx_3.success = true;
    executed_tx_3.tx_hash = vec![1, 1, 2, 30];
    executed_tx_3.created_at = timestamp_2 - Duration::weeks(1);
    // Store them.
    storage
        .chain()
        .operations_schema()
        .store_executed_tx(executed_tx_1)
        .await?;
    storage
        .chain()
        .operations_schema()
        .store_executed_tx(executed_tx_2)
        .await?;
    storage
        .chain()
        .operations_schema()
        .store_executed_tx(executed_tx_3)
        .await?;
    // First check, no transactions removed.
    storage
        .chain()
        .operations_schema()
        .remove_rejected_transactions(Duration::weeks(3))
        .await?;
    let block_number = BlockNumber(0);
    let count = storage
        .chain()
        .stats_schema()
        .count_outstanding_proofs(block_number)
        .await?;
    assert_eq!(count, 3);
    // Second transaction should be removed.
    storage
        .chain()
        .operations_schema()
        .remove_rejected_transactions(Duration::days(10))
        .await?;
    let count = storage
        .chain()
        .stats_schema()
        .count_outstanding_proofs(block_number)
        .await?;
    assert_eq!(count, 2);
    // Finally, no rejected transactions remain.
    storage
        .chain()
        .operations_schema()
        .remove_rejected_transactions(Duration::days(4))
        .await?;
    let count = storage
        .chain()
        .stats_schema()
        .count_outstanding_proofs(block_number)
        .await?;
    assert_eq!(count, 1);
    // The last one is indeed succesful.
    let count = storage
        .chain()
        .stats_schema()
        .count_total_transactions()
        .await?;
    assert_eq!(count, 1);

    Ok(())
}

/// Checks if executed_priority_operations are removed correctly.
#[db_test]
async fn test_remove_executed_priority_operations(
    mut storage: StorageProcessor<'_>,
) -> QueryResult<()> {
    // Insert 5 priority operations.
    for block_number in 1..=5 {
        let executed_priority_op = NewExecutedPriorityOperation {
            block_number,
            block_index: 1,
            operation: Default::default(),
            from_account: Default::default(),
            to_account: Default::default(),
            priority_op_serialid: block_number,
            deadline_block: 100,
            eth_hash: vec![0xDE, 0xAD, 0xBE, 0xEF],
            eth_block: 10,
            created_at: chrono::Utc::now(),
        };
        OperationsSchema(&mut storage)
            .store_executed_priority_op(executed_priority_op)
            .await?;
    }

    // Remove priority operation with block numbers greater than 3.
    OperationsSchema(&mut storage)
        .remove_executed_priority_operations(BlockNumber(3))
        .await?;

    // Check that priority operation from the 3rd block is present and from the 4th is not.
    let block3_txs = BlockSchema(&mut storage)
        .get_block_transactions(BlockNumber(3))
        .await?;
    assert!(!block3_txs.is_empty());

    let block4_txs = BlockSchema(&mut storage)
        .get_block_transactions(BlockNumber(4))
        .await?;
    assert!(block4_txs.is_empty());

    Ok(())
}

/// Checks if ethereum unprocessed aggregated operations are removed correctly.
#[db_test]
async fn test_remove_eth_unprocessed_aggregated_ops(
    mut storage: StorageProcessor<'_>,
) -> QueryResult<()> {
    let block_number = 1;
    let action_type = AggregatedActionType::CommitBlocks;
    // Save commit aggregated operation.
    OperationsSchema(&mut storage)
        .store_aggregated_action(gen_unique_aggregated_operation(
            BlockNumber(block_number),
            action_type,
            100,
        ))
        .await?;
    // Add this operation to eth_unprocessed_aggregated_ops table.
    storage
        .ethereum_schema()
        .restore_unprocessed_operations()
        .await?;
    // Remove ethereum unprocessed aggregated operations.
    OperationsSchema(&mut storage)
        .remove_eth_unprocessed_aggregated_ops()
        .await?;
    let unprocessed_op_count = storage
        .ethereum_schema()
        .load_unconfirmed_operations()
        .await?
        .len();
    assert_eq!(unprocessed_op_count, 0);

    Ok(())
}
