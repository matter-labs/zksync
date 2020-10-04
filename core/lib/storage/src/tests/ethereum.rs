// Built-in deps
use std::str::FromStr;
// External imports
use zksync_basic_types::{H256, U256};
// Workspace imports
use zksync_crypto::Fr;
use zksync_types::{
    ethereum::{ETHOperation, OperationType},
    Action, Operation,
    {block::Block, BlockNumber},
};
// Local imports
use crate::tests::db_test;
use crate::{chain::block::BlockSchema, ethereum::EthereumSchema, QueryResult, StorageProcessor};
use num::BigUint;

/// Creates a sample operation to be stored in `operations` table.
/// This function is required since `eth_operations` table is linked to
/// the `operations` table by the operation id.
pub fn get_commit_operation(block_number: BlockNumber) -> Operation {
    Operation {
        id: None,
        action: Action::Commit,
        block: Block::new(
            block_number,
            Fr::default(),
            0,
            Vec::new(),
            (0, 0),
            100,
            1_000_000.into(),
            1_500_000.into(),
        ),
    }
}

/// Same as `get_commit_operation`, but creates a verify operation instead.
pub fn get_verify_operation(block_number: BlockNumber) -> Operation {
    let action = Action::Verify {
        proof: Default::default(),
    };
    Operation {
        id: None,
        action,
        block: Block::new(
            block_number,
            Fr::default(),
            0,
            Vec::new(),
            (0, 0),
            100,
            1_000_000.into(),
            1_500_000.into(),
        ),
    }
}

/// Parameters for `EthereumSchema::save_operation_eth_tx` method.
#[derive(Debug)]
pub struct EthereumTxParams {
    op_type: String,
    op: Operation,
    hash: H256,
    deadline_block: u64,
    gas_price: BigUint,
    raw_tx: Vec<u8>,
}

impl EthereumTxParams {
    pub fn new(op_type: String, op: Operation) -> Self {
        let op_id = op.id.unwrap() as u64;
        Self {
            op_type,
            op,
            hash: H256::from_low_u64_ne(op_id),
            deadline_block: 100,
            gas_price: 1000u32.into(),
            raw_tx: Default::default(),
        }
    }

    pub fn to_eth_op(&self, db_id: i64, nonce: u64) -> ETHOperation {
        let op_type = OperationType::from_str(self.op_type.as_ref())
            .expect("Stored operation type must have a valid value");
        let last_used_gas_price = U256::from_str(&self.gas_price.to_string()).unwrap();
        let used_tx_hashes = vec![self.hash];

        ETHOperation {
            id: db_id,
            op_type,
            op: Some(self.op.clone()),
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

    let unconfirmed_operations = EthereumSchema(&mut storage)
        .load_unconfirmed_operations()
        .await?;
    assert!(unconfirmed_operations.is_empty());

    // Store operation with ID 1.
    let block_number = 1;
    let operation = BlockSchema(&mut storage)
        .execute_operation(get_commit_operation(block_number))
        .await?;

    // Store the Ethereum transaction.
    let params = EthereumTxParams::new("commit".into(), operation.clone());
    let response = EthereumSchema(&mut storage)
        .save_new_eth_tx(
            OperationType::Commit,
            Some(params.op.id.unwrap()),
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
    let op = eth_op.op.clone().expect("No Operation entry");
    assert_eq!(op.id, operation.id);
    // Load the database ID, since we can't predict it for sure.
    assert_eq!(
        eth_op,
        params.to_eth_op(eth_op.id, response.nonce.low_u64())
    );

    // Store operation with ID 2.
    let block_number = 2;
    let operation_2 = BlockSchema(&mut storage)
        .execute_operation(get_commit_operation(block_number))
        .await?;

    // Create one more Ethereum transaction.
    let params_2 = EthereumTxParams::new("commit".into(), operation_2.clone());
    let response_2 = EthereumSchema(&mut storage)
        .save_new_eth_tx(
            OperationType::Commit,
            Some(params_2.op.id.unwrap()),
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
    let op = eth_op.op.clone().expect("No Operation entry");
    assert_eq!(op.id, operation_2.id);
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

    assert_eq!(updated_stats.commit_ops, 2);
    assert_eq!(updated_stats.verify_ops, 0);
    assert_eq!(updated_stats.withdraw_ops, 0);

    Ok(())
}

/// Check that stored nonce starts with 0 and is incremented after every getting.
#[db_test]
async fn eth_nonce(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    EthereumSchema(&mut storage).initialize_eth_data().await?;

    for expected_next_nonce in 0..5 {
        let actual_next_nonce = EthereumSchema(&mut storage).get_next_nonce().await?;

        assert_eq!(actual_next_nonce, expected_next_nonce);
    }

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
    let block_number = 1;
    let operation = BlockSchema(&mut storage)
        .execute_operation(get_commit_operation(block_number))
        .await?;
    let verify_operation = BlockSchema(&mut storage)
        .execute_operation(get_verify_operation(block_number))
        .await?;

    // Now there must be one unprocessed operation.
    let unprocessed_operations = EthereumSchema(&mut storage)
        .load_unprocessed_operations()
        .await?;
    assert_eq!(unprocessed_operations.len(), 2);
    assert_eq!(unprocessed_operations[0].id, operation.id);
    assert_eq!(unprocessed_operations[1].id, verify_operation.id);

    // Check that it's not currently returned by `load_unconfirmed_operations`.
    let unconfirmed_operations = EthereumSchema(&mut storage)
        .load_unconfirmed_operations()
        .await?;
    assert!(unconfirmed_operations.is_empty());

    // Store the Ethereum transaction.
    let params = EthereumTxParams::new("commit".into(), operation.clone());
    let response = EthereumSchema(&mut storage)
        .save_new_eth_tx(
            OperationType::Commit,
            Some(params.op.id.unwrap()),
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
    let op = eth_op.op.clone().expect("No Operation entry");
    assert_eq!(op.id, operation.id);
    // Load the database ID, since we can't predict it for sure.
    assert_eq!(
        eth_op,
        params.to_eth_op(eth_op.id, response.nonce.low_u64())
    );

    // After we created an ETHOperation for the operation, there must be no unprocessed operations.
    let unprocessed_operations = EthereumSchema(&mut storage)
        .load_unprocessed_operations()
        .await?;
    assert_eq!(unprocessed_operations.len(), 1);
    assert_eq!(unprocessed_operations[0].id, verify_operation.id);

    let verify_params = EthereumTxParams::new("verify".into(), verify_operation.clone());
    let response = EthereumSchema(&mut storage)
        .save_new_eth_tx(
            OperationType::Verify,
            Some(verify_params.op.id.unwrap()),
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
