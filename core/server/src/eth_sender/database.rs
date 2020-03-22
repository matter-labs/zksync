//! Module encapsulating the database interaction.
//! The essential part of this module is the trait that abstracts
//! the database interaction, so `ETHSender` won't require an actual
//! database to run, which is required for tests.

// Built-in deps
use std::collections::VecDeque;
use std::str::FromStr;
// External uses
use bigdecimal::BigDecimal;
use web3::types::H256;
// Workspace uses
use storage::ConnectionPool;
// Local uses
use super::transactions::{OperationETHState, TransactionETHState};

/// Abstract database access trait, optimized for the needs of `ETHSender`.
pub(super) trait DatabaseAccess {
    /// Loads the unconfirmed operations from the database.
    fn restore_state(&self) -> Result<VecDeque<OperationETHState>, failure::Error>;

    /// Saves an unconfirmed operation to the database.
    fn save_unconfirmed_operation(&self, tx: &TransactionETHState) -> Result<(), failure::Error>;

    /// Marks an operation as completed in the database.
    fn confirm_operation(&self, hash: &H256) -> Result<(), failure::Error>;
}

/// The actual database wrapper.
/// This structure uses `ConnectionPool` to interact with an existing database.
pub struct Database {
    /// Connection to the database.
    db_pool: ConnectionPool,
}

impl Database {
    pub fn new(db_pool: ConnectionPool) -> Self {
        Self { db_pool }
    }
}

impl DatabaseAccess for Database {
    fn restore_state(&self) -> Result<VecDeque<OperationETHState>, failure::Error> {
        let storage = self
            .db_pool
            .access_storage()
            .expect("Failed to access storage");

        let unconfirmed_ops = storage
            .ethereum_schema()
            .load_unconfirmed_operations()?
            .into_iter()
            .map(|(operation, txs)| OperationETHState {
                operation,
                txs: txs.into_iter().map(|tx| tx.into()).collect(),
            })
            .collect();
        Ok(unconfirmed_ops)
    }

    fn save_unconfirmed_operation(&self, tx: &TransactionETHState) -> Result<(), failure::Error> {
        let storage = self.db_pool.access_storage()?;
        Ok(storage.ethereum_schema().save_operation_eth_tx(
            tx.op_id,
            tx.signed_tx.hash,
            tx.deadline_block,
            tx.signed_tx.nonce.as_u32(),
            BigDecimal::from_str(&tx.signed_tx.gas_price.to_string()).unwrap(),
            tx.signed_tx.raw_tx.clone(),
        )?)
    }

    fn confirm_operation(&self, hash: &H256) -> Result<(), failure::Error> {
        let storage = self.db_pool.access_storage()?;
        Ok(storage.ethereum_schema().confirm_eth_tx(hash)?)
    }
}
