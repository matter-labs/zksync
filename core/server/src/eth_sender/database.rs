//! Module encapsulating the database interaction.
//! The essential part of this module is the trait that abstracts
//! the database interaction, so `ETHSender` won't require an actual
//! database to run, which is required for tests.

// Built-in deps
use std::collections::VecDeque;
use std::str::FromStr;
// External uses
use bigdecimal::BigDecimal;
use web3::types::{H256, U256};
// Workspace uses
use models::{
    ethereum::{ETHOperation, EthOpId},
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
    fn save_new_eth_tx(&self, op: &ETHOperation) -> Result<EthOpId, failure::Error>;

    /// Adds a new tx info to the previously started Ethereum operation.
    fn update_eth_tx(
        &self,
        eth_op_id: EthOpId,
        hash: &H256,
        new_deadline_block: i64,
        new_gas_value: U256,
    ) -> Result<(), failure::Error>;

    /// Marks an operation as completed in the database.
    fn confirm_operation(&self, hash: &H256) -> Result<(), failure::Error>;

    /// Gets the next nonce to use from the database.
    fn next_nonce(&self) -> Result<i64, failure::Error>;

    /// Loads the stored Ethereum operations stats.
    fn load_stats(&self) -> Result<ETHStats, failure::Error>;
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
    fn restore_state(&self) -> Result<(VecDeque<ETHOperation>, Vec<Operation>), failure::Error> {
        let storage = self
            .db_pool
            .access_storage()
            .expect("Failed to access storage");

        let unconfirmed_ops = storage.ethereum_schema().load_unconfirmed_operations()?;
        let unprocessed_ops = storage.ethereum_schema().load_unprocessed_operations()?;
        Ok((unconfirmed_ops, unprocessed_ops))
    }

    fn save_new_eth_tx(&self, op: &ETHOperation) -> Result<EthOpId, failure::Error> {
        let storage = self.db_pool.access_storage()?;

        assert_eq!(
            op.used_tx_hashes.len(),
            1,
            "For the new operation there should be exactly one tx hash"
        );
        let tx_hash = op.used_tx_hashes[0];
        Ok(storage.ethereum_schema().save_new_eth_tx(
            op.op_type.clone(),
            op.op.clone().map(|op| op.id.unwrap()),
            tx_hash,
            op.last_deadline_block,
            op.nonce.as_u32(),
            BigDecimal::from_str(&op.last_used_gas_price.to_string()).unwrap(),
            op.encoded_tx_data.clone(),
        )?)
    }

    fn update_eth_tx(
        &self,
        eth_op_id: EthOpId,
        hash: &H256,
        new_deadline_block: i64,
        new_gas_value: U256,
    ) -> Result<(), failure::Error> {
        let storage = self.db_pool.access_storage()?;
        Ok(storage.ethereum_schema().update_eth_tx(
            eth_op_id,
            hash,
            new_deadline_block,
            BigDecimal::from_str(&new_gas_value.to_string()).unwrap(),
        )?)
    }

    fn confirm_operation(&self, hash: &H256) -> Result<(), failure::Error> {
        let storage = self.db_pool.access_storage()?;
        Ok(storage.ethereum_schema().confirm_eth_tx(hash)?)
    }

    fn next_nonce(&self) -> Result<i64, failure::Error> {
        let storage = self.db_pool.access_storage()?;
        Ok(storage.ethereum_schema().get_next_nonce()?)
    }

    fn load_stats(&self) -> Result<ETHStats, failure::Error> {
        let storage = self.db_pool.access_storage()?;
        let stats = storage.ethereum_schema().load_stats()?;
        Ok(stats.into())
    }
}
