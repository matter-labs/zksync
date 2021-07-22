// Built-in deps
use std::str::FromStr;
// External imports
use zksync_basic_types::{H256, U256};
// Workspace imports
use zksync_types::{
    aggregated_operations::{AggregatedActionType, AggregatedOperation},
    ethereum::ETHOperation,
    BlockNumber,
};
// Local imports
use crate::test_data::{gen_unique_aggregated_operation, BLOCK_SIZE_CHUNKS};
use crate::tests::db_test;
use crate::{
    chain::operations::OperationsSchema, ethereum::EthereumSchema, QueryResult, StorageProcessor,
};
use num::BigUint;

/// Parameters for `EthereumSchema::save_operation_eth_tx` method.
#[derive(Debug)]
pub struct EthereumTxParams {
    op_type: String,
    op: Option<(i64, AggregatedOperation)>,
    hash: H256,
    deadline_block: u64,
    gas_price: BigUint,
    raw_tx: Vec<u8>,
}

impl EthereumTxParams {
    pub fn new(op_type: String, op: Option<(i64, AggregatedOperation)>) -> Self {
        let op_id = op.clone().map(|(id, _)| id).unwrap_or_default();

        Self {
            op_type,
            op,
            hash: H256::from_low_u64_ne(op_id as u64),
            deadline_block: 100,
            gas_price: 1000u32.into(),
            raw_tx: Default::default(),
        }
    }

    pub fn to_eth_op(&self, db_id: i64, nonce: u64) -> ETHOperation {
        let op_type = AggregatedActionType::from_str(self.op_type.as_ref())
            .expect("Stored operation type must have a valid value");
        let last_used_gas_price = U256::from_str(&self.gas_price.to_string()).unwrap();
        let used_tx_hashes = vec![self.hash];

        ETHOperation {
            id: db_id,
            op_type,
            op: self.op.clone(),
            nonce: nonce.into(),
            last_deadline_block: self.deadline_block,
            last_used_gas_price,
            used_tx_hashes,
            encoded_tx_data: self.raw_tx.clone(),
            confirmed: false,
            final_hash: None,
        }
    }
}

/// Verifies that on a fresh database no bogus operations are loaded.
#[db_test]
async fn ethereum_empty_load(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let unconfirmed_operations = EthereumSchema(&mut storage)
        .load_unconfirmed_operations()
        .await?;
    assert!(unconfirmed_operations.is_empty());

    Ok(())
}

/// Checks the basic Ethereum storage workflow:
/// - Store the operations in the block schema.
/// - Save the Ethereum tx.
/// - Check that saved tx can be loaded.
/// - Save another Ethereum tx for the same operation.
/// - Check that both txs can be loaded.
/// - Make the operation as completed.
/// - Check that now txs aren't loaded.
#[db_test]
async fn ethereum_storage(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    EthereumSchema(&mut storage).initialize_eth_data().await?;

    for expected_next_nonce in 0..5 {
        let actual_next_nonce = EthereumSchema(&mut storage).get_next_nonce().await?;

        assert_eq!(actual_next_nonce, expected_next_nonce);
    }

    let unconfirmed_operations = EthereumSchema(&mut storage)
        .load_unconfirmed_operations()
        .await?;
    assert!(unconfirmed_operations.is_empty());

    // Store operation with ID 1.
    let block_number = BlockNumber(1);
    OperationsSchema(&mut storage)
        .store_aggregated_action(gen_unique_aggregated_operation(
            block_number,
            AggregatedActionType::CommitBlocks,
            BLOCK_SIZE_CHUNKS,
        ))
        .await?;
    let op = OperationsSchema(&mut storage)
        .get_aggregated_op_that_affects_block(AggregatedActionType::CommitBlocks, block_number)
        .await?;

    // Store the Ethereum transaction.
    let params = EthereumTxParams::new("CommitBlocks".into(), op);
    let response = EthereumSchema(&mut storage)
        .save_new_eth_tx(
            AggregatedActionType::CommitBlocks,
            params.op.clone(),
            params.deadline_block as i64,
            params.gas_price.clone(),
            params.raw_tx.clone(),
        )
        .await?;
    EthereumSchema(&mut storage)
        .add_hash_entry(response.id, &params.hash)
        .await?;

    // Check that it can be loaded.
    let unconfirmed_operations = EthereumSchema(&mut storage)
        .load_unconfirmed_operations()
        .await?;
    let eth_op = unconfirmed_operations[0].clone();
    // assert_eq!(Some(op.0), operation.id);
    // Load the database ID, since we can't predict it for sure.
    assert_eq!(
        eth_op,
        params.to_eth_op(eth_op.id, response.nonce.low_u64())
    );

    // Store operation with ID 2.
    let block_number = BlockNumber(2);
    OperationsSchema(&mut storage)
        .store_aggregated_action(gen_unique_aggregated_operation(
            block_number,
            AggregatedActionType::CreateProofBlocks,
            BLOCK_SIZE_CHUNKS,
        ))
        .await?;
    let op = OperationsSchema(&mut storage)
        .get_aggregated_op_that_affects_block(AggregatedActionType::CreateProofBlocks, block_number)
        .await?;

    // Create one more Ethereum transaction.
    let params_2 = EthereumTxParams::new("CommitBlocks".into(), op);
    let response_2 = EthereumSchema(&mut storage)
        .save_new_eth_tx(
            AggregatedActionType::CreateProofBlocks,
            params_2.op.clone(),
            params_2.deadline_block as i64,
            params_2.gas_price.clone(),
            params_2.raw_tx.clone(),
        )
        .await?;
    EthereumSchema(&mut storage)
        .add_hash_entry(response_2.id, &params_2.hash)
        .await?;

    // Check that we now can load two operations.
    let unconfirmed_operations = EthereumSchema(&mut storage)
        .load_unconfirmed_operations()
        .await?;
    assert_eq!(unconfirmed_operations.len(), 2);
    let eth_op = unconfirmed_operations[1].clone();
    // assert_eq!(op.id, operation_2.id);
    assert_eq!(
        eth_op,
        params_2.to_eth_op(eth_op.id, response_2.nonce.low_u64())
    );

    // Make the transaction as completed.
    EthereumSchema(&mut storage)
        .confirm_eth_tx(&params_2.hash)
        .await?;

    // Now there should be only one unconfirmed operation.
    let unconfirmed_operations = EthereumSchema(&mut storage)
        .load_unconfirmed_operations()
        .await?;
    assert_eq!(unconfirmed_operations.len(), 1);

    // Check that stats are updated as well.
    let updated_stats = EthereumSchema(&mut storage).load_stats().await?;

    assert_eq!(updated_stats.last_committed_block, 1);
    assert_eq!(updated_stats.last_verified_block, 0);
    assert_eq!(updated_stats.last_executed_block, 0);

    Ok(())
}

/// Here we check `unprocessed` and `unconfirmed` operations getting.
/// If there is no `ETHOperation` for `Operation`, it must be returend by `load_unprocessed_operations`.
/// It must **not** be returned by `load_unconfirmed_operations`.
///
/// If there is an `ETHOperation` and it's not confirmed, it must be returned by `load_unconfirmed_operations`
/// and **not** returned by `load_unprocessed_operations`.
#[db_test]
async fn ethereum_unprocessed(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    EthereumSchema(&mut storage).initialize_eth_data().await?;

    let unconfirmed_operations = EthereumSchema(&mut storage)
        .load_unconfirmed_operations()
        .await?;
    assert!(unconfirmed_operations.is_empty());

    let unprocessed_operations = EthereumSchema(&mut storage)
        .load_unprocessed_operations()
        .await?;
    assert!(unprocessed_operations.is_empty());

    // Store operation with ID 1.
    let block_number = BlockNumber(1);
    OperationsSchema(&mut storage)
        .store_aggregated_action(gen_unique_aggregated_operation(
            block_number,
            AggregatedActionType::CommitBlocks,
            BLOCK_SIZE_CHUNKS,
        ))
        .await?;
    let commit_operation = OperationsSchema(&mut storage)
        .get_aggregated_op_that_affects_block(AggregatedActionType::CommitBlocks, block_number)
        .await?;
    OperationsSchema(&mut storage)
        .store_aggregated_action(gen_unique_aggregated_operation(
            block_number,
            AggregatedActionType::PublishProofBlocksOnchain,
            BLOCK_SIZE_CHUNKS,
        ))
        .await?;
    let verify_operation = OperationsSchema(&mut storage)
        .get_aggregated_op_that_affects_block(
            AggregatedActionType::PublishProofBlocksOnchain,
            block_number,
        )
        .await?;

    // Now there must be one unprocessed operation.
    let unprocessed_operations = EthereumSchema(&mut storage)
        .load_unprocessed_operations()
        .await?;
    assert_eq!(unprocessed_operations.len(), 2);
    assert_eq!(
        unprocessed_operations[0].0,
        commit_operation.as_ref().unwrap().0
    );
    assert_eq!(
        unprocessed_operations[1].0,
        verify_operation.as_ref().unwrap().0
    );

    // Check that it's not currently returned by `load_unconfirmed_operations`.
    let unconfirmed_operations = EthereumSchema(&mut storage)
        .load_unconfirmed_operations()
        .await?;
    assert!(unconfirmed_operations.is_empty());

    // Store the Ethereum transaction.
    let params = EthereumTxParams::new("CommitBlocks".into(), commit_operation.clone());
    let response = EthereumSchema(&mut storage)
        .save_new_eth_tx(
            AggregatedActionType::CommitBlocks,
            params.op.clone(),
            params.deadline_block as i64,
            params.gas_price.clone(),
            params.raw_tx.clone(),
        )
        .await?;
    EthereumSchema(&mut storage)
        .add_hash_entry(response.id, &params.hash)
        .await?;

    // Check that it can be loaded.
    let unconfirmed_operations = EthereumSchema(&mut storage)
        .load_unconfirmed_operations()
        .await?;
    assert_eq!(unconfirmed_operations.len(), 1);
    let eth_op = unconfirmed_operations[0].clone();
    // assert_eq!(op.id, operation.id);
    // Load the database ID, since we can't predict it for sure.
    assert_eq!(
        eth_op,
        params.to_eth_op(eth_op.id, response.nonce.low_u64())
    );

    // After we created an ETHOperation for the operation, the number of unprocessed operations should not change.
    let unprocessed_operations = EthereumSchema(&mut storage)
        .load_unprocessed_operations()
        .await?;
    assert_eq!(unprocessed_operations.len(), 2);
    assert_eq!(
        unprocessed_operations[0].0,
        commit_operation.as_ref().unwrap().0
    );
    assert_eq!(
        unprocessed_operations[1].0,
        verify_operation.as_ref().unwrap().0
    );

    // let's mark the operations as successful processed.
    // So that next time you do not add them to the queue again.
    let operations_id = unprocessed_operations
        .iter()
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();
    EthereumSchema(&mut storage)
        .remove_unprocessed_operations(operations_id)
        .await?;

    // Check that unprocessed operations have been deleted.
    let unprocessed_operations = EthereumSchema(&mut storage)
        .load_unprocessed_operations()
        .await?;
    assert_eq!(unprocessed_operations.len(), 0);

    let verify_params =
        EthereumTxParams::new("PublishProofBlocksOnchain".into(), verify_operation.clone());
    let response = EthereumSchema(&mut storage)
        .save_new_eth_tx(
            AggregatedActionType::PublishProofBlocksOnchain,
            verify_params.op,
            verify_params.deadline_block as i64,
            verify_params.gas_price.clone(),
            verify_params.raw_tx.clone(),
        )
        .await?;
    EthereumSchema(&mut storage)
        .add_hash_entry(response.id, &verify_params.hash)
        .await?;

    let unprocessed_operations = EthereumSchema(&mut storage)
        .load_unprocessed_operations()
        .await?;
    assert!(unprocessed_operations.is_empty());

    let unconfirmed_operations = EthereumSchema(&mut storage)
        .load_unconfirmed_operations()
        .await?;
    assert_eq!(unconfirmed_operations.len(), 2);

    // Confirm first tx and check that it isn't returned by `unconfirmed` method anymore.
    EthereumSchema(&mut storage)
        .confirm_eth_tx(&params.hash)
        .await?;

    let unconfirmed_operations = EthereumSchema(&mut storage)
        .load_unconfirmed_operations()
        .await?;
    assert_eq!(unconfirmed_operations.len(), 1);

    Ok(())
}

/// Simple test for store/load of (average) gas price.
#[db_test]
async fn ethereum_gas_update(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    storage.ethereum_schema().initialize_eth_data().await?;
    let old_price_limit = storage.ethereum_schema().load_gas_price_limit().await?;
    let old_average_price = storage.ethereum_schema().load_average_gas_price().await?;
    // This parameter is not set in `initialize_eth_data()`
    assert!(old_average_price.is_none());
    // Update these values.
    storage
        .ethereum_schema()
        .update_gas_price(old_price_limit + 1i32, old_price_limit - 1i32)
        .await?;
    // Load new ones.
    let new_price_limit = storage.ethereum_schema().load_gas_price_limit().await?;
    let new_average_price = storage.ethereum_schema().load_average_gas_price().await?;

    assert_eq!(new_price_limit, old_price_limit + 1i32);
    assert_eq!(new_average_price, Some(old_price_limit - 1i32));

    Ok(())
}

/// Check update eth parameters
#[db_test]
async fn test_update_eth_parameters(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    storage.ethereum_schema().initialize_eth_data().await?;

    // Updates eth parameters and checks if they were really saved.
    storage
        .ethereum_schema()
        .update_eth_parameters(BlockNumber(5))
        .await?;

    let stats = storage.ethereum_schema().load_stats().await?;
    assert_eq!(stats.last_committed_block, 5);
    assert_eq!(stats.last_verified_block, 0);
    assert_eq!(stats.last_executed_block, 0);

    Ok(())
}
