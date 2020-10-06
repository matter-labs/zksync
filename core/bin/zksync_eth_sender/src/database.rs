//! Module encapsulating the database interaction.
//! The essential part of this module is the trait that abstracts
//! the database interaction, so `ETHSender` won't require an actual
//! database to run, which is required for tests.

// Built-in deps
use std::collections::VecDeque;
use std::str::FromStr;
// External uses
use num::BigUint;
use zksync_basic_types::{H256, U256};
// Workspace uses
use zksync_storage::{ConnectionPool, StorageProcessor};
use zksync_types::{
    ethereum::{ETHOperation, EthOpId, InsertedOperationResponse, OperationType},
    Operation,
};
// Local uses
use super::transactions::ETHStats;

/// The actual database wrapper.
/// This structure uses `StorageProcessor` to interact with an existing database.
#[derive(Debug)]
pub struct Database {
    /// Connection to the database.
    db_pool: ConnectionPool,
}

impl Database {
    pub fn new(db_pool: ConnectionPool) -> Self {
        Self { db_pool }
    }
}

impl Database {
    pub async fn acquire_connection(&self) -> Result<StorageProcessor<'_>, anyhow::Error> {
        let connection = self.db_pool.access_storage().await?;

        Ok(connection)
    }

    pub async fn restore_state(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> Result<(VecDeque<ETHOperation>, Vec<Operation>), anyhow::Error> {
        let unconfirmed_ops = connection
            .ethereum_schema()
            .load_unconfirmed_operations()
            .await?;
        let unprocessed_ops = connection
            .ethereum_schema()
            .load_unprocessed_operations()
            .await?;
        Ok((unconfirmed_ops, unprocessed_ops))
    }

    pub async fn load_new_operations(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> Result<Vec<Operation>, anyhow::Error> {
        let unprocessed_ops = connection
            .ethereum_schema()
            .load_unprocessed_operations()
            .await?;
        Ok(unprocessed_ops)
    }

    pub async fn save_new_eth_tx(
        &self,
        connection: &mut StorageProcessor<'_>,
        op_type: OperationType,
        op: Option<Operation>,
        deadline_block: i64,
        used_gas_price: U256,
        raw_tx: Vec<u8>,
    ) -> Result<InsertedOperationResponse, anyhow::Error> {
        let result = connection
            .ethereum_schema()
            .save_new_eth_tx(
                op_type,
                op.map(|op| op.id.unwrap()),
                deadline_block,
                BigUint::from_str(&used_gas_price.to_string()).unwrap(),
                raw_tx,
            )
            .await?;

        Ok(result)
    }

    pub async fn add_hash_entry(
        &self,
        connection: &mut StorageProcessor<'_>,
        eth_op_id: i64,
        hash: &H256,
    ) -> Result<(), anyhow::Error> {
        Ok(connection
            .ethereum_schema()
            .add_hash_entry(eth_op_id, hash)
            .await?)
    }

    pub async fn update_eth_tx(
        &self,
        connection: &mut StorageProcessor<'_>,
        eth_op_id: EthOpId,
        new_deadline_block: i64,
        new_gas_value: U256,
    ) -> Result<(), anyhow::Error> {
        Ok(connection
            .ethereum_schema()
            .update_eth_tx(
                eth_op_id,
                new_deadline_block,
                BigUint::from_str(&new_gas_value.to_string()).unwrap(),
            )
            .await?)
    }

    pub async fn confirm_operation(
        &self,
        connection: &mut StorageProcessor<'_>,
        hash: &H256,
    ) -> Result<(), anyhow::Error> {
        Ok(connection.ethereum_schema().confirm_eth_tx(hash).await?)
    }

    pub async fn load_stats(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> Result<ETHStats, anyhow::Error> {
        let stats = connection.ethereum_schema().load_stats().await?;
        Ok(stats.into())
    }

    pub async fn load_gas_price_limit(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> Result<U256, anyhow::Error> {
        let limit = connection.ethereum_schema().load_gas_price_limit().await?;
        Ok(limit)
    }

    pub async fn update_gas_price_params(
        &self,
        connection: &mut StorageProcessor<'_>,
        gas_price_limit: U256,
        average_gas_price: U256,
    ) -> Result<(), anyhow::Error> {
        connection
            .ethereum_schema()
            .update_gas_price(gas_price_limit, average_gas_price)
            .await?;
        Ok(())
    }
}
