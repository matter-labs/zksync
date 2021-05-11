// Built-in deps
use std::{collections::VecDeque, convert::TryFrom, str::FromStr, time::Instant};
// External imports
use anyhow::format_err;
use num::{BigInt, BigUint};
use sqlx::types::BigDecimal;
use zksync_basic_types::{H256, U256};
// Workspace imports
use zksync_types::{
    aggregated_operations::{AggregatedActionType, AggregatedOperation},
    ethereum::{ETHOperation, InsertedOperationResponse},
    event::{
        account::AccountStateChangeStatus, block::BlockStatus, transaction::TransactionStatus,
    },
    BlockNumber,
};
// Local imports
use self::records::{ETHParams, ETHStats, ETHTxHash, StorageETHOperation};
use crate::{chain::operations::records::StoredAggregatedOperation, QueryResult, StorageProcessor};

pub mod records;

/// Ethereum schema is capable of storing the information about the
/// interaction with the Ethereum blockchain (mainly the list of sent
/// Ethereum transactions).
#[derive(Debug)]
pub struct EthereumSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> EthereumSchema<'a, 'c> {
    /// Loads the list of operations that were not confirmed on Ethereum,
    /// each operation has a list of sent Ethereum transactions.
    pub async fn load_unconfirmed_operations(&mut self) -> QueryResult<VecDeque<ETHOperation>> {
        let start = Instant::now();
        // Load the operations with the associated Ethereum transactions
        // from the database.
        // Here we obtain a sequence of one-to-one mappings (ETH tx) -> (operation ID).
        // Each Ethereum transaction can have no more than one associated operation, and each
        // operation is associated with exactly one Ethereum transaction. Note that there may
        // be ETH transactions without an operation (e.g. `completeWithdrawals` call), but for
        // every operation always there is an ETH transaction.

        let mut transaction = self.0.start_transaction().await?;

        // TODO: Currently `sqlx` doesn't work well with joins, thus we will perform one additional query
        // for each loaded operation. This is not crucial, as this operation is done once per node launch,
        // but not effective and must be fixed as soon as `sqlx` 0.5 is released (ZKS-102).
        let eth_ops = sqlx::query_as!(
            StorageETHOperation,
            "SELECT * FROM eth_operations
            WHERE confirmed = false
            ORDER BY id ASC"
        )
        .fetch_all(transaction.conn())
        .await?;

        // Create a vector for the expected output.
        let mut ops: VecDeque<ETHOperation> = VecDeque::with_capacity(eth_ops.len());

        // Transform the `StoredOperation` to `Operation` and `StoredETHOperation` to `ETHOperation`.
        for eth_op in eth_ops {
            let raw_op = sqlx::query_as!(
                StoredAggregatedOperation,
                r#"
                SELECT aggregate_operations.* FROM eth_aggregated_ops_binding
                LEFT JOIN aggregate_operations ON aggregate_operations.id = op_id
                WHERE eth_op_id = $1
                "#,
                eth_op.id
            )
            .fetch_optional(transaction.conn())
            .await?;

            // Load the stored txs hashes ordered by their ID,
            // so the latest added hash will be the last one in the list.
            let eth_tx_hashes: Vec<ETHTxHash> = sqlx::query_as!(
                ETHTxHash,
                "SELECT * FROM eth_tx_hashes
                WHERE eth_op_id = $1
                ORDER BY id ASC",
                eth_op.id
            )
            .fetch_all(transaction.conn())
            .await?;
            assert!(
                !eth_tx_hashes.is_empty(),
                "No hashes stored for the Ethereum operation"
            );

            // If there is an operation, convert it to the `Operation` type.
            let op = raw_op.map(|raw| raw.into_aggregated_op());

            // Convert the fields into expected format.
            let op_type = AggregatedActionType::from_str(eth_op.op_type.as_ref())
                .expect("Stored operation type must have a valid value");
            let last_used_gas_price =
                U256::from_str(&eth_op.last_used_gas_price.to_string()).unwrap();
            let used_tx_hashes = eth_tx_hashes
                .iter()
                .map(|entry| H256::from_slice(&entry.tx_hash))
                .collect();
            let final_hash = eth_op.final_hash.map(|hash| H256::from_slice(&hash));

            let eth_op = ETHOperation {
                id: eth_op.id,
                op_type,
                op,
                nonce: eth_op.nonce.into(),
                last_deadline_block: eth_op.last_deadline_block as u64,
                last_used_gas_price,
                used_tx_hashes,
                encoded_tx_data: eth_op.raw_tx,
                confirmed: eth_op.confirmed,
                final_hash,
            };

            ops.push_back(eth_op);
        }

        transaction.commit().await?;

        metrics::histogram!("sql.ethereum.load_unconfirmed_operations", start.elapsed());
        Ok(ops)
    }

    /// Load all the aggregated operations that have no confirmation yet and have not yet been sent to Ethereum.
    /// Should be used after server restart only.
    pub async fn restore_unprocessed_operations(&mut self) -> QueryResult<()> {
        let start = Instant::now();

        sqlx::query!(
            "WITH aggregate_ops AS (
                SELECT aggregate_operations.id FROM aggregate_operations
                   WHERE confirmed = $1 and action_type != $2 and aggregate_operations.id != ANY(SELECT id from eth_aggregated_ops_binding)
                ORDER BY aggregate_operations.id ASC
              )
              INSERT INTO eth_unprocessed_aggregated_ops (op_id)
              SELECT id from aggregate_ops
              ON CONFLICT (op_id)
              DO NOTHING",
              false,
              AggregatedActionType::CreateProofBlocks.to_string()
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.ethereum.restore_unprocessed_operations",
            start.elapsed()
        );

        Ok(())
    }

    /// Loads the operations which were stored in `aggregate_operations` table,
    /// and are in `eth_unprocessed_aggregated_ops`.
    pub async fn load_unprocessed_operations(
        &mut self,
    ) -> QueryResult<Vec<(i64, AggregatedOperation)>> {
        let start = Instant::now();

        let raw_ops = sqlx::query_as!(
            StoredAggregatedOperation,
            r#"
            SELECT * FROM aggregate_operations
            WHERE EXISTS (SELECT * FROM eth_unprocessed_aggregated_ops WHERE op_id = aggregate_operations.id)
            ORDER BY id ASC
            "#,
        )
        .fetch_all(self.0.conn())
        .await?;

        let mut operations = Vec::new();

        for raw_op in raw_ops {
            // We filtered operations that don't have Ethereum binding right in the SQL query,
            // so now we only have to convert stored operations into `Operation`.
            let op = raw_op.into_aggregated_op();
            if !matches!(
                op.1.get_action_type(),
                AggregatedActionType::CreateProofBlocks
            ) {
                operations.push(op);
            }
        }

        metrics::histogram!("sql.ethereum.load_unprocessed_operations", start.elapsed());
        Ok(operations)
    }

    /// Removes the given IDs from `eth_unprocessed_aggregated_ops`.
    /// Used to indicate that operations have been successfully processed.
    pub async fn remove_unprocessed_operations(
        &mut self,
        operations_id: Vec<i64>,
    ) -> QueryResult<()> {
        let start = Instant::now();

        sqlx::query!(
            "DELETE FROM eth_unprocessed_aggregated_ops WHERE op_id = ANY($1)",
            &operations_id
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.ethereum.remove_unprocessed_operations",
            start.elapsed()
        );
        Ok(())
    }

    /// Stores the sent (but not confirmed yet) Ethereum transaction in the database.
    /// Returns the `ETHOperation` object containing the assigned nonce and operation ID.
    pub async fn save_new_eth_tx(
        &mut self,
        op_type: AggregatedActionType,
        operation: Option<(i64, AggregatedOperation)>,
        last_deadline_block: i64,
        last_used_gas_price: BigUint,
        raw_tx: Vec<u8>,
    ) -> QueryResult<InsertedOperationResponse> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        // It's important to assign nonce within the same db transaction
        // as saving the operation to avoid the state divergence.
        let nonce = EthereumSchema(&mut transaction).get_next_nonce().await?;

        // Create and insert the operation.

        // Obtain the operation ID for the follow-up queried.
        let last_used_gas_price = BigDecimal::from(BigInt::from(last_used_gas_price));
        let eth_op_id = sqlx::query!(
            "
                INSERT INTO eth_operations (op_type, nonce, last_deadline_block, last_used_gas_price, raw_tx)
                VALUES ($1, $2, $3, $4, $5)
                RETURNING id
            ",
            op_type.to_string(), nonce, last_deadline_block, last_used_gas_price, raw_tx,
        )
        .fetch_one(transaction.conn())
        .await?
        .id;

        // If the operation ID was provided, we should also insert a binding entry.
        if let Some((op_id, op)) = operation {
            sqlx::query!(
                "INSERT INTO eth_aggregated_ops_binding (op_id, eth_op_id) VALUES ($1, $2)",
                op_id,
                eth_op_id
            )
            .execute(transaction.conn())
            .await?;

            // Update the stored stats.
            EthereumSchema(&mut transaction)
                .report_created_operation(op)
                .await?;
        }

        // Return the assigned ID and nonce.
        let response = InsertedOperationResponse {
            id: eth_op_id,
            nonce: nonce.into(),
        };

        transaction.commit().await?;

        metrics::histogram!("sql.ethereum.save_new_eth_tx", start.elapsed());
        Ok(response)
    }

    /// Returns whether the operation with the given id was confirmed.
    /// If the operation with such id does not exist, then it returns Ok(false).
    pub async fn is_aggregated_op_confirmed(&mut self, id: i64) -> QueryResult<bool> {
        let start = Instant::now();
        let confirmed = sqlx::query_as!(
            StorageETHOperation,
            "SELECT * FROM eth_operations WHERE id <= $1 ORDER BY ID DESC LIMIT 1",
            id
        )
        .fetch_optional(self.0.conn())
        .await?
        .map(|op| op.confirmed)
        .unwrap_or(false);

        metrics::histogram!("sql.ethereum.is_aggregated_op_confirmed", start.elapsed());
        Ok(confirmed)
    }

    /// Retrieves the Ethereum operation ID given the tx hash.
    async fn get_eth_op_id(&mut self, hash: &H256) -> QueryResult<i64> {
        let start = Instant::now();
        let hash_entry = sqlx::query_as!(
            ETHTxHash,
            "SELECT * FROM eth_tx_hashes WHERE tx_hash = $1",
            hash.as_bytes()
        )
        .fetch_one(self.0.conn())
        .await?;

        metrics::histogram!("sql.ethereum.get_eth_op_id", start.elapsed());
        Ok(hash_entry.eth_op_id)
    }

    /// Adds a tx hash entry associated with some Ethereum operation to the database.
    pub async fn add_hash_entry(&mut self, eth_op_id: i64, hash: &H256) -> QueryResult<()> {
        let start = Instant::now();
        // Insert the new hash entry.
        sqlx::query!(
            "INSERT INTO eth_tx_hashes (eth_op_id, tx_hash) VALUES ($1, $2)",
            eth_op_id,
            hash.as_bytes()
        )
        .execute(self.0.conn())
        .await?;
        metrics::histogram!("sql.ethereum.add_hash_entry", start.elapsed());
        Ok(())
    }

    /// Updates the Ethereum operation by adding a new tx data.
    /// The new deadline block / gas value are placed instead of old values to the main entry.
    pub async fn update_eth_tx(
        &mut self,
        eth_op_id: i64,
        new_deadline_block: i64,
        new_gas_value: BigUint,
    ) -> QueryResult<()> {
        let start = Instant::now();
        // Update the stored tx.
        let new_gas_price = BigDecimal::from(BigInt::from(new_gas_value));
        sqlx::query!(
            "UPDATE eth_operations 
            SET last_used_gas_price = $1, last_deadline_block = $2
            WHERE id = $3",
            new_gas_price,
            new_deadline_block,
            eth_op_id
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.ethereum.update_eth_tx", start.elapsed());
        Ok(())
    }

    /// Updates the stats counter with the new operation reported.
    /// This method should be called once **per operation**. It means that if transaction
    /// for some operation was stuck, and another transaction was created for it, this method
    /// **should not** be invoked.
    ///
    /// This method expects the database to be initially prepared with inserting the actual
    /// stats values. Currently the script `db-insert-eth-data.sh` is responsible for that
    /// and it's invoked within `db-reset` subcommand.
    async fn report_created_operation(
        &mut self,
        operation: AggregatedOperation,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let mut current_stats = EthereumSchema(&mut transaction).load_eth_params().await?;
        let (first_block, last_block) = {
            let block_range = operation.get_block_range();
            (i64::from(*block_range.0), i64::from(*block_range.1))
        };

        match operation {
            AggregatedOperation::CommitBlocks(_) => {
                if current_stats.last_committed_block + 1 != first_block {
                    return Err(format_err!(
                        "Report created commit not in ascending order of affected blocks"
                    ));
                }
                current_stats.last_committed_block = last_block;
            }
            AggregatedOperation::PublishProofBlocksOnchain(_) => {
                if current_stats.last_verified_block + 1 != first_block {
                    return Err(format_err!(
                        "Report published proof not in ascending order of affected blocks"
                    ));
                }
                current_stats.last_verified_block = last_block;
            }
            AggregatedOperation::ExecuteBlocks(_) => {
                if current_stats.last_executed_block + 1 != first_block {
                    return Err(format_err!(
                        "Report executed blocks not in ascending order of affected blocks"
                    ));
                }
                current_stats.last_executed_block = last_block;
            }
            AggregatedOperation::CreateProofBlocks(_) => return Ok(()),
        };

        // Update the stored stats.
        sqlx::query!(
            "UPDATE eth_parameters
            SET last_committed_block = $1, last_verified_block = $2, last_executed_block = $3
            WHERE id = true",
            current_stats.last_committed_block,
            current_stats.last_verified_block,
            current_stats.last_executed_block
        )
        .execute(transaction.conn())
        .await?;

        transaction.commit().await?;

        metrics::histogram!("sql.ethereum.report_created_operation", start.elapsed());
        Ok(())
    }

    /// Updates the stored gas price limit and average gas price used by GasAdjuster.
    ///
    /// This method expects the database to be initially prepared with inserting the actual
    /// gas limit value. The command responsible for that is `zk db insert eth-data`.
    pub async fn update_gas_price(
        &mut self,
        gas_price_limit: U256,
        average_gas_price: U256,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let gas_price_limit: i64 =
            i64::try_from(gas_price_limit).expect("Can't convert U256 to i64");
        let average_gas_price: i64 =
            i64::try_from(average_gas_price).expect("Can't convert U256 to i64");

        // Update the stored gas price limit.
        sqlx::query!(
            "UPDATE eth_parameters
            SET gas_price_limit = $1, average_gas_price = $2
            WHERE id = true",
            gas_price_limit,
            average_gas_price
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.ethereum.update_gas_price", start.elapsed());
        Ok(())
    }

    pub async fn load_gas_price_limit(&mut self) -> QueryResult<U256> {
        let start = Instant::now();
        let params = self.load_eth_params().await?;

        let gas_price_limit =
            U256::try_from(params.gas_price_limit).expect("Negative gas limit value stored in DB");

        metrics::histogram!("sql.ethereum.load_gas_price_limit", start.elapsed());
        Ok(gas_price_limit)
    }

    pub async fn load_average_gas_price(&mut self) -> QueryResult<Option<U256>> {
        let start = Instant::now();
        let params = self.load_eth_params().await?;

        let average_gas_price = params
            .average_gas_price
            .map(|price| U256::try_from(price).expect("Negative average gas price stored in DB"));

        metrics::histogram!("sql.ethereum.load_average_gas_price", start.elapsed());
        Ok(average_gas_price)
    }

    /// Loads the stored Ethereum operations stats.
    pub async fn load_stats(&mut self) -> QueryResult<ETHStats> {
        let start = Instant::now();
        let params = self.load_eth_params().await?;

        metrics::histogram!("sql.ethereum.load_stats", start.elapsed());
        Ok(params.into())
    }

    async fn load_eth_params(&mut self) -> QueryResult<ETHParams> {
        let start = Instant::now();
        let params = sqlx::query_as!(ETHParams, "SELECT * FROM eth_parameters WHERE id = true",)
            .fetch_one(self.0.conn())
            .await?;
        metrics::histogram!("sql.ethereum.load_eth_params", start.elapsed());
        Ok(params)
    }

    /// Marks the stored Ethereum transaction as confirmed (and thus the associated `Operation`
    /// is marked as confirmed as well).
    pub async fn confirm_eth_tx(&mut self, hash: &H256) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let eth_op_id = EthereumSchema(&mut transaction).get_eth_op_id(hash).await?;

        // Set the `confirmed` and `final_hash` field of the entry.
        sqlx::query!(
            "UPDATE eth_operations
                SET confirmed = $1, final_hash = $2
                WHERE id = $3",
            true,
            hash.as_bytes(),
            eth_op_id
        )
        .execute(transaction.conn())
        .await?;

        // If there is a ZKSync operation, mark it as confirmed as well.
        let aggregated_op = sqlx::query_as!(
            StoredAggregatedOperation,
            "SELECT * FROM aggregate_operations
                WHERE id = (SELECT op_id FROM eth_aggregated_ops_binding WHERE eth_op_id = $1)",
            eth_op_id,
        )
        .fetch_optional(transaction.conn())
        .await?;

        if let Some(op) = &aggregated_op {
            let (from_block, to_block) = (op.from_block as u32, op.to_block as u32);
            let action_type = AggregatedActionType::from_str(&op.action_type).unwrap();
            transaction
                .chain()
                .operations_schema()
                .confirm_aggregated_operations(
                    BlockNumber(from_block),
                    BlockNumber(to_block),
                    action_type,
                )
                .await?;

            let status = AccountStateChangeStatus::try_from(action_type).ok();
            if let Some(status) = status {
                let block_status = BlockStatus::from(status);
                let block_operations_status = TransactionStatus::from(status);
                // Store events about the block, corresponding account updates and
                // executed operations.
                for block_number in from_block..=to_block {
                    transaction
                        .event_schema()
                        .store_block_event(BlockNumber(block_number), block_status)
                        .await?;
                    transaction
                        .event_schema()
                        .store_state_updated_event(BlockNumber(block_number), status)
                        .await?;
                    transaction
                        .event_schema()
                        .store_transaction_event(BlockNumber(block_number), block_operations_status)
                        .await?;
                }
            }
        }

        transaction.commit().await?;

        metrics::histogram!("sql.ethereum.confirm_eth_tx", start.elapsed());
        Ok(())
    }

    /// Obtains the next nonce to use and updates the corresponding entry in the database
    /// for the next invocation.
    ///
    /// This method expects the database to be initially prepared with inserting the actual
    /// nonce value. Currently the script `db-insert-eth-data.sh` is responsible for that
    /// and it's invoked within `db-reset` subcommand.
    #[doc = "hidden"]
    pub async fn get_next_nonce(&mut self) -> QueryResult<i64> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let old_nonce: ETHParams = EthereumSchema(&mut transaction).load_eth_params().await?;

        let new_nonce_value = old_nonce.nonce + 1;

        sqlx::query!(
            "UPDATE eth_parameters
            SET nonce = $1
            WHERE id = true",
            new_nonce_value
        )
        .execute(transaction.conn())
        .await?;

        let old_nonce_value = old_nonce.nonce;

        transaction.commit().await?;

        metrics::histogram!("sql.ethereum.get_next_nonce", start.elapsed());
        Ok(old_nonce_value)
    }

    /// Method that internally initializes the `eth_parameters` table.
    /// Since in db tests the database is empty, we must provide a possibility
    /// to initialize required db fields.
    #[doc = "hidden"]
    pub async fn initialize_eth_data(&mut self) -> QueryResult<()> {
        let start = Instant::now();
        #[derive(Debug)]
        pub struct NewETHParams {
            pub nonce: i64,
            pub gas_price_limit: i64,
            pub last_committed_block: i64,
            pub last_verified_block: i64,
            pub last_executed_block: i64,
        }

        let old_params: Option<ETHParams> =
            sqlx::query_as!(ETHParams, "SELECT * FROM eth_parameters WHERE id = true",)
                .fetch_optional(self.0.conn())
                .await?;

        if old_params.is_none() {
            let params = NewETHParams {
                nonce: 0,
                gas_price_limit: 400 * 10e9 as i64,
                last_committed_block: 0,
                last_verified_block: 0,
                last_executed_block: 0,
            };

            sqlx::query!(
                "INSERT INTO eth_parameters (nonce, gas_price_limit, last_committed_block, last_verified_block, last_executed_block)
                VALUES ($1, $2, $3, $4, $5)",
                params.nonce, params.gas_price_limit, params.last_committed_block, params.last_verified_block, params.last_executed_block
            )
            .execute(self.0.conn())
            .await?;
        }

        metrics::histogram!("sql.ethereum.initialize_eth_data", start.elapsed());
        Ok(())
    }

    pub async fn aggregated_op_final_hash(
        &mut self,
        block_number: BlockNumber,
    ) -> QueryResult<Option<H256>> {
        let eth_operation = sqlx::query_as!(
            StorageETHOperation,
            "SELECT eth_operations.* FROM aggregate_operations
                LEFT JOIN eth_aggregated_ops_binding ON eth_aggregated_ops_binding.op_id = aggregate_operations.id
                LEFT JOIN eth_operations ON eth_aggregated_ops_binding.eth_op_id = eth_operations.id
            WHERE
                ($1 BETWEEN from_block AND to_block) AND action_type = $2 AND eth_operations.confirmed = true 
            LIMIT 1",
            i64::from(*block_number),
            AggregatedActionType::ExecuteBlocks.to_string(),
        )
        .fetch_optional(self.0.conn())
        .await?;

        let final_hash = eth_operation
            .map(|eth_operation| eth_operation.final_hash.map(|hash| H256::from_slice(&hash)))
            .flatten();

        Ok(final_hash)
    }

    // Updates eth_parameters with given nonce and last block.
    // It updates last_verified_block only if it is greater than given last block.
    pub async fn update_eth_parameters(&mut self, last_block: BlockNumber) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;
        sqlx::query!(
            "UPDATE eth_parameters SET last_committed_block = $1 WHERE id = true",
            *last_block as i64
        )
        .execute(transaction.conn())
        .await?;

        sqlx::query!(
            "UPDATE eth_parameters SET last_verified_block = $1 WHERE id = true AND last_verified_block > $1",
            *last_block as i64
        )
        .execute(transaction.conn())
        .await?;
        transaction.commit().await?;

        metrics::histogram!("sql.ethereum.update_eth_parameters", start.elapsed());
        Ok(())
    }
}
