//! Module encapsulating the database interaction.
//! The essential part of this module is the trait that abstracts
//! the database interaction, so `ETHSender` won't require an actual
//! database to run, which is required for tests.

// Built-in deps
use std::collections::VecDeque;
use std::str::FromStr;
// External uses
use num::BigUint;
use web3::types::{H256, U256};
// Workspace uses
use models::{
    ethereum::{ETHOperation, EthOpId, InsertedOperationResponse, OperationType},
    Operation,
};
use storage::ConnectionPool;
// Local uses
use super::transactions::ETHStats;

/// Abstract database access trait, optimized for the needs of `ETHSender`.
pub(super) trait DatabaseAccess {
    /// Loads the unconfirmed and unprocessed operations from the database.
    /// Unconfirmed operations are Ethereum operations that were started, but not confirmed yet.
    /// Unprocessed operations are ZK Sync operations that were not started at all.
    fn restore_state(&self) -> Result<(VecDeque<ETHOperation>, Vec<Operation>), failure::Error>;

    /// Saves a new unconfirmed operation to the database.
    fn save_new_eth_tx(
        &self,
        op_type: OperationType,
        op_id: Option<Operation>,
        deadline_block: i64,
        used_gas_price: U256,
        raw_tx: Vec<u8>,
    ) -> Result<InsertedOperationResponse, failure::Error>;

    /// Adds a tx hash entry associated with some Ethereum operation to the database.
    fn add_hash_entry(&self, eth_op_id: i64, hash: &H256) -> Result<(), failure::Error>;

    /// Adds a new tx info to the previously started Ethereum operation.
    fn update_eth_tx(
        &self,
        eth_op_id: EthOpId,
        new_deadline_block: i64,
        new_gas_value: U256,
    ) -> Result<(), failure::Error>;

    /// Marks an operation as completed in the database.
    fn confirm_operation(&self, hash: &H256) -> Result<(), failure::Error>;

    /// Loads the stored Ethereum operations stats.
    fn load_stats(&self) -> Result<ETHStats, failure::Error>;

    /// Performs several database operations within one database transaction.
    fn transaction<F, T>(&self, f: F) -> Result<T, failure::Error>
    where
        F: FnOnce() -> Result<T, failure::Error>;
}

/// The actual database wrapper.
/// This structure uses `ConnectionPool` to interact with an existing database.
#[derive(Debug, Clone)]
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
    fn restore_state(&self) -> Result<(VecDeque<ETHOperation>, Vec<Operation>), failure::Error> {
        let storage = self
            .db_pool
            .access_storage()
            .expect("Failed to access storage");

        let unconfirmed_ops = storage.ethereum_schema().load_unconfirmed_operations()?;
        let unprocessed_ops = storage.ethereum_schema().load_unprocessed_operations()?;
        Ok((unconfirmed_ops, unprocessed_ops))
    }

    fn save_new_eth_tx(
        &self,
        op_type: OperationType,
        op: Option<Operation>,
        deadline_block: i64,
        used_gas_price: U256,
        raw_tx: Vec<u8>,
    ) -> Result<InsertedOperationResponse, failure::Error> {
        let storage = self.db_pool.access_storage()?;

        Ok(storage.ethereum_schema().save_new_eth_tx(
            op_type,
            op.map(|op| op.id.unwrap()),
            deadline_block,
            BigUint::from_str(&used_gas_price.to_string()).unwrap(),
            raw_tx,
        )?)
    }

    fn add_hash_entry(&self, eth_op_id: i64, hash: &H256) -> Result<(), failure::Error> {
        let storage = self.db_pool.access_storage()?;

        Ok(storage.ethereum_schema().add_hash_entry(eth_op_id, hash)?)
    }

    fn update_eth_tx(
        &self,
        eth_op_id: EthOpId,
        new_deadline_block: i64,
        new_gas_value: U256,
    ) -> Result<(), failure::Error> {
        let storage = self.db_pool.access_storage()?;
        Ok(storage.ethereum_schema().update_eth_tx(
            eth_op_id,
            new_deadline_block,
            BigUint::from_str(&new_gas_value.to_string()).unwrap(),
        )?)
    }

    fn confirm_operation(&self, hash: &H256) -> Result<(), failure::Error> {
        let storage = self.db_pool.access_storage()?;
        Ok(storage.ethereum_schema().confirm_eth_tx(hash)?)
    }

    fn load_stats(&self) -> Result<ETHStats, failure::Error> {
        let storage = self.db_pool.access_storage()?;
        let stats = storage.ethereum_schema().load_stats()?;
        Ok(stats.into())
    }

    fn transaction<F, T>(&self, f: F) -> Result<T, failure::Error>
    where
        F: FnOnce() -> Result<T, failure::Error>,
    {
        let storage = self.db_pool.access_storage()?;

        storage.transaction(|| f())
    }
}
