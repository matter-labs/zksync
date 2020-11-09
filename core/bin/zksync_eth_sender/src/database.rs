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
    pub async fn acquire_connection(&self) -> anyhow::Result<StorageProcessor<'_>> {
        let connection = self.db_pool.access_storage().await?;

        Ok(connection)
    }

    pub async fn restore_state(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<(VecDeque<ETHOperation>, Vec<Operation>)> {
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
    ) -> anyhow::Result<Vec<Operation>> {
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
    ) -> anyhow::Result<InsertedOperationResponse> {
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
    ) -> anyhow::Result<()> {
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
    ) -> anyhow::Result<()> {
        Ok(connection
            .ethereum_schema()
            .update_eth_tx(
                eth_op_id,
                new_deadline_block,
                BigUint::from_str(&new_gas_value.to_string()).unwrap(),
            )
            .await?)
    }

    pub async fn is_previous_operation_confirmed(
        &self,
        connection: &mut StorageProcessor<'_>,
        op: &ETHOperation,
    ) -> anyhow::Result<bool> {
        let confirmed = match op.op_type {
            OperationType::Commit | OperationType::Verify => {
                let op = op.op.as_ref().unwrap();
                // We're checking previous block, so for the edge case of first block we can say that it was confirmed.
                let block_to_check = if op.block.block_number > 1 {
                    op.block.block_number - 1
                } else {
                    return Ok(true);
                };

                let maybe_operation = connection
                    .chain()
                    .operations_schema()
                    .get_operation(block_to_check, op.action.get_type())
                    .await;
                let operation = match maybe_operation {
                    Some(op) => op,
                    None => return Ok(false),
                };
                operation.confirmed
            }
            OperationType::Withdraw => {
                // Withdrawals aren't actually sequential, so we don't really care.
                true
            }
        };

        Ok(confirmed)
    }

    pub async fn confirm_operation(
        &self,
        connection: &mut StorageProcessor<'_>,
        hash: &H256,
        op: &ETHOperation,
    ) -> anyhow::Result<()> {
        if let OperationType::Verify = op.op_type {
            let mut transaction = connection.start_transaction().await?;

            transaction.ethereum_schema().confirm_eth_tx(hash).await?;
            transaction
                .chain()
                .state_schema()
                .apply_state_update(op.op.as_ref().unwrap().block.block_number)
                .await?;

            transaction.commit().await?;
        } else {
            connection.ethereum_schema().confirm_eth_tx(hash).await?;
        }
        Ok(())
    }

    pub async fn load_stats(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<ETHStats> {
        let stats = connection.ethereum_schema().load_stats().await?;
        Ok(stats.into())
    }

    pub async fn load_gas_price_limit(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<U256> {
        let limit = connection.ethereum_schema().load_gas_price_limit().await?;
        Ok(limit)
    }

    pub async fn update_gas_price_params(
        &self,
        connection: &mut StorageProcessor<'_>,
        gas_price_limit: U256,
        average_gas_price: U256,
    ) -> anyhow::Result<()> {
        connection
            .ethereum_schema()
            .update_gas_price(gas_price_limit, average_gas_price)
            .await?;
        Ok(())
    }
}
