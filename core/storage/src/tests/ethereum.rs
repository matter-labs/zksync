// External imports
use bigdecimal::BigDecimal;
use web3::types::H256;
// Workspace imports
use models::{
    node::{block::Block, BlockNumber, Fr},
    Action, Operation,
};
// Local imports
use crate::tests::db_test;
use crate::{
    chain::block::BlockSchema,
    ethereum::{records::StorageETHOperation, EthereumSchema},
    StorageProcessor,
};

/// Creates a sample operation to be stored in `operations` table.
/// This function is required since `eth_operations` table is linked to
/// the `operations` table by the operation id.
pub fn get_operation(block_number: BlockNumber) -> Operation {
    Operation {
        id: None,
        action: Action::Commit,
        block: Block {
            block_number,
            new_root_hash: Fr::default(),
            fee_account: 0,
            block_transactions: Vec::new(),
            processed_priority_ops: (0, 0),
        },
        accounts_updated: Default::default(),
    }
}

/// Parameters for `EthereumSchema::save_operation_eth_tx` method.
#[derive(Debug)]
pub struct EthereumTxParams {
    op_id: i64,
    hash: H256,
    deadline_block: u64,
    nonce: u32,
    gas_price: BigDecimal,
    raw_tx: Vec<u8>,
}

impl EthereumTxParams {
    pub fn new(op_id: i64, nonce: u32) -> Self {
        Self {
            op_id,
            hash: H256::from_low_u64_ne(op_id as u64),
            deadline_block: 100,
            nonce,
            gas_price: 1000.into(),
            raw_tx: Default::default(),
        }
    }

    pub fn to_eth_op(&self, db_id: i64) -> StorageETHOperation {
        StorageETHOperation {
            id: db_id,
            op_id: self.op_id,
            nonce: self.nonce as i64,
            deadline_block: self.deadline_block as i64,
            gas_price: self.gas_price.clone(),
            tx_hash: self.hash.as_bytes().to_vec(),
            confirmed: false,
            raw_tx: self.raw_tx.clone(),
        }
    }
}

/// Verifies that on a fresh database no bogus operations are loaded.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn ethereum_empty_load() {
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        let unconfirmed_operations = EthereumSchema(&conn).load_unconfirmed_operations()?;
        assert!(unconfirmed_operations.is_empty());

        Ok(())
    });
}

/// Checks the basic Ethereum storage workflow:
/// - Store the operations in the block schema.
/// - Save the Ethereum tx.
/// - Check that saved tx can be loaded.
/// - Save another Ethereum tx for the same operation.
/// - Check that both txs can be loaded.
/// - Make the operation as completed.
/// - Check that now txs aren't loaded.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn ethereum_storage() {
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        let unconfirmed_operations = EthereumSchema(&conn).load_unconfirmed_operations()?;
        assert!(unconfirmed_operations.is_empty());

        // Store operation with ID 1.
        let block_number = 1;
        let operation = BlockSchema(&conn).execute_operation(get_operation(block_number))?;

        // Store the Ethereum transaction.
        let params = EthereumTxParams::new(operation.id.unwrap(), 1);
        EthereumSchema(&conn).save_operation_eth_tx(
            params.op_id,
            params.hash,
            params.deadline_block,
            params.nonce,
            params.gas_price.clone(),
            params.raw_tx.clone(),
        )?;

        // Check that it can be loaded.
        let unconfirmed_operations = EthereumSchema(&conn).load_unconfirmed_operations()?;
        assert_eq!(unconfirmed_operations[0].0.id, operation.id);
        assert_eq!(unconfirmed_operations[0].1.len(), 1);
        // Load the database ID, since we can't predict it for sure.
        let db_id = unconfirmed_operations[0].1[0].id;
        assert_eq!(unconfirmed_operations[0].1, vec![params.to_eth_op(db_id)]);

        // Create one more Ethereum transaction.
        let params_2 = EthereumTxParams::new(operation.id.unwrap(), 2);
        EthereumSchema(&conn).save_operation_eth_tx(
            params_2.op_id,
            params_2.hash,
            params_2.deadline_block,
            params_2.nonce,
            params_2.gas_price.clone(),
            params_2.raw_tx.clone(),
        )?;

        // Check that we now can load two operations.
        let unconfirmed_operations = EthereumSchema(&conn).load_unconfirmed_operations()?;
        assert_eq!(unconfirmed_operations[0].0.id, operation.id);
        assert_eq!(unconfirmed_operations[0].1.len(), 2);
        let db_id_2 = unconfirmed_operations[0].1[1].id;
        assert_eq!(
            unconfirmed_operations[0].1,
            vec![params.to_eth_op(db_id), params_2.to_eth_op(db_id_2)]
        );

        // Make the transaction as completed.
        EthereumSchema(&conn).confirm_eth_tx(&params_2.hash)?;

        // Now there should be no unconfirmed transactions.
        let unconfirmed_operations = EthereumSchema(&conn).load_unconfirmed_operations()?;
        assert!(unconfirmed_operations.is_empty());

        Ok(())
    });
}
