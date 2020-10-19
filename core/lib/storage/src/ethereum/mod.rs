// Built-in deps
use std::{collections::VecDeque, convert::TryFrom, str::FromStr};
// External imports
use num::{BigInt, BigUint};
use sqlx::types::BigDecimal;
use zksync_basic_types::{H256, U256};
// Workspace imports
use zksync_types::{
    ethereum::{ETHOperation, InsertedOperationResponse, OperationType},
    Operation,
};
// Local imports
use self::records::{ETHParams, ETHStats, ETHTxHash, StorageETHOperation};
use crate::chain::operations::records::StoredOperation;
use crate::{QueryResult, StorageProcessor};

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
        // but not effective and must be fixed as soon as `sqlx` 0.5 is released.
        // Details on issue: https://github.com/launchbadge/sqlx/issues/367
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
                StoredOperation,
                "SELECT operations.* FROM eth_ops_binding
                LEFT JOIN operations ON operations.id = op_id
                WHERE eth_op_id = $1",
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
            let op = if let Some(raw_op) = raw_op {
                Some(raw_op.into_op(&mut transaction).await?)
            } else {
                None
            };

            // Convert the fields into expected format.
            let op_type = OperationType::from_str(eth_op.op_type.as_ref())
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

        Ok(ops)
    }

    /// Loads the operations which were stored in `operations` table, but not
    /// in the `eth_operations`. This method is intended to be used after relaunch
    /// to synchronize `eth_sender` state, as operations are sent to the `eth_sender`
    /// only once.
    pub async fn load_unprocessed_operations(&mut self) -> QueryResult<Vec<Operation>> {
        let mut transaction = self.0.start_transaction().await?;

        let raw_ops = sqlx::query_as!(
            StoredOperation,
            "SELECT * FROM operations
            WHERE confirmed = false AND NOT EXISTS (SELECT * FROM eth_ops_binding WHERE op_id = operations.id)
            ORDER BY id ASC",
        )
        .fetch_all(transaction.conn())
        .await?;

        let mut operations: Vec<Operation> = Vec::new();

        for raw_op in raw_ops {
            // We filtered operations that don't have Ethereum binding right in the SQL query,
            // so now we only have to convert stored operations into `Operation`.
            let op = raw_op
                .into_op(&mut transaction)
                .await
                .expect("Can't convert the operation");
            operations.push(op);
        }

        transaction.commit().await?;

        Ok(operations)
    }

    /// Stores the sent (but not confirmed yet) Ethereum transaction in the database.
    /// Returns the `ETHOperation` object containing the assigned nonce and operation ID.
    pub async fn save_new_eth_tx(
        &mut self,
        op_type: OperationType,
        op_id: Option<i64>,
        last_deadline_block: i64,
        last_used_gas_price: BigUint,
        raw_tx: Vec<u8>,
    ) -> QueryResult<InsertedOperationResponse> {
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

        // // Add a hash entry.
        // let hash_entry = NewETHTxHash {
        //     eth_op_id,
        //     tx_hash: hash.as_bytes().to_vec(),
        // };
        // let inserted_hashes_rows = insert_into(eth_tx_hashes::table)
        //     .values(&hash_entry)
        //     .execute(self.0.conn())?;
        // assert_eq!(
        //     inserted_hashes_rows, 1,
        //     "Wrong amount of updated rows (eth_tx_hashes)"
        // );

        // If the operation ID was provided, we should also insert a binding entry.
        if let Some(op_id) = op_id {
            sqlx::query!(
                "INSERT INTO eth_ops_binding (op_id, eth_op_id) VALUES ($1, $2)",
                op_id,
                eth_op_id
            )
            .execute(transaction.conn())
            .await?;
        }

        // Update the stored stats.
        EthereumSchema(&mut transaction)
            .report_created_operation(op_type)
            .await?;

        // Return the assigned ID and nonce.
        let response = InsertedOperationResponse {
            id: eth_op_id,
            nonce: nonce.into(),
        };

        transaction.commit().await?;

        Ok(response)
    }

    /// Retrieves the Ethereum operation ID given the tx hash.
    async fn get_eth_op_id(&mut self, hash: &H256) -> QueryResult<i64> {
        let hash_entry = sqlx::query_as!(
            ETHTxHash,
            "SELECT * FROM eth_tx_hashes WHERE tx_hash = $1",
            hash.as_bytes()
        )
        .fetch_one(self.0.conn())
        .await?;

        Ok(hash_entry.eth_op_id)
    }

    /// Adds a tx hash entry associated with some Ethereum operation to the database.
    pub async fn add_hash_entry(&mut self, eth_op_id: i64, hash: &H256) -> QueryResult<()> {
        // Insert the new hash entry.
        sqlx::query!(
            "INSERT INTO eth_tx_hashes (eth_op_id, tx_hash) VALUES ($1, $2)",
            eth_op_id,
            hash.as_bytes()
        )
        .execute(self.0.conn())
        .await?;
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
    async fn report_created_operation(&mut self, operation_type: OperationType) -> QueryResult<()> {
        let mut transaction = self.0.start_transaction().await?;

        let mut current_stats = EthereumSchema(&mut transaction).load_eth_params().await?;

        // Increase the only one type of operations.
        match operation_type {
            OperationType::Commit => {
                current_stats.commit_ops += 1;
            }
            OperationType::Verify => {
                current_stats.verify_ops += 1;
            }
            OperationType::Withdraw => {
                current_stats.withdraw_ops += 1;
            }
        };

        // Update the stored stats.
        sqlx::query!(
            "UPDATE eth_parameters
            SET commit_ops = $1, verify_ops = $2, withdraw_ops = $3
            WHERE id = true",
            current_stats.commit_ops,
            current_stats.verify_ops,
            current_stats.withdraw_ops
        )
        .execute(transaction.conn())
        .await?;

        transaction.commit().await?;

        Ok(())
    }

    /// Updates the stored gas price limit and average gas price used by GasAdjuster.
    ///
    /// This method expects the database to be initially prepared with inserting the actual
    /// gas limit value. Currently the script `db-insert-eth-data.sh` is responsible for that
    /// and it's invoked within `db-reset` subcommand.
    pub async fn update_gas_price(
        &mut self,
        gas_price_limit: U256,
        average_gas_price: U256,
    ) -> QueryResult<()> {
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

        Ok(())
    }

    pub async fn load_gas_price_limit(&mut self) -> QueryResult<U256> {
        let params = self.load_eth_params().await?;

        let gas_price_limit =
            U256::try_from(params.gas_price_limit).expect("Negative gas limit value stored in DB");

        Ok(gas_price_limit)
    }

    pub async fn load_average_gas_price(&mut self) -> QueryResult<Option<U256>> {
        let params = self.load_eth_params().await?;

        let average_gas_price = params
            .average_gas_price
            .map(|price| U256::try_from(price).expect("Negative average gas price stored in DB"));

        Ok(average_gas_price)
    }

    /// Loads the stored Ethereum operations stats.
    pub async fn load_stats(&mut self) -> QueryResult<ETHStats> {
        let params = self.load_eth_params().await?;

        Ok(params.into())
    }

    async fn load_eth_params(&mut self) -> QueryResult<ETHParams> {
        let params = sqlx::query_as!(ETHParams, "SELECT * FROM eth_parameters WHERE id = true",)
            .fetch_one(self.0.conn())
            .await?;
        Ok(params)
    }

    /// Marks the stored Ethereum transaction as confirmed (and thus the associated `Operation`
    /// is marked as confirmed as well).
    pub async fn confirm_eth_tx(&mut self, hash: &H256) -> QueryResult<()> {
        let mut transaction = self.0.start_transaction().await?;

        let eth_op_id = EthereumSchema(&mut transaction).get_eth_op_id(hash).await?;

        // Set the `confirmed` and `final_hash` field of the entry.
        let eth_op_id: i64 = sqlx::query!(
            "UPDATE eth_operations
                SET confirmed = $1, final_hash = $2
                WHERE id = $3
                RETURNING id",
            true,
            hash.as_bytes(),
            eth_op_id
        )
        .fetch_one(transaction.conn())
        .await?
        .id;

        // If there is a ZKSync operation, mark it as confirmed as well.
        sqlx::query!(
            "
            UPDATE operations
                SET confirmed = $1
                WHERE id = (SELECT op_id FROM eth_ops_binding WHERE eth_op_id = $2)",
            true,
            eth_op_id,
        )
        .execute(transaction.conn())
        .await?;

        transaction.commit().await?;

        Ok(())
    }

    /// Obtains the next nonce to use and updates the corresponding entry in the database
    /// for the next invocation.
    ///
    /// This method expects the database to be initially prepared with inserting the actual
    /// nonce value. Currently the script `db-insert-eth-data.sh` is responsible for that
    /// and it's invoked within `db-reset` subcommand.
    pub(crate) async fn get_next_nonce(&mut self) -> QueryResult<i64> {
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

        Ok(old_nonce_value)
    }

    /// Method that internally initializes the `eth_parameters` table.
    /// Since in db tests the database is empty, we must provide a possibility
    /// to initialize required db fields.
    #[cfg(test)]
    pub async fn initialize_eth_data(&mut self) -> QueryResult<()> {
        #[derive(Debug)]
        pub struct NewETHParams {
            pub nonce: i64,
            pub gas_price_limit: i64,
            pub commit_ops: i64,
            pub verify_ops: i64,
            pub withdraw_ops: i64,
        }

        let old_params: Option<ETHParams> =
            sqlx::query_as!(ETHParams, "SELECT * FROM eth_parameters WHERE id = true",)
                .fetch_optional(self.0.conn())
                .await?;

        if old_params.is_none() {
            let params = NewETHParams {
                nonce: 0,
                gas_price_limit: 400 * 10e9 as i64,
                commit_ops: 0,
                verify_ops: 0,
                withdraw_ops: 0,
            };

            sqlx::query!(
                "INSERT INTO eth_parameters (nonce, gas_price_limit, commit_ops, verify_ops, withdraw_ops)
                VALUES ($1, $2, $3, $4, $5)",
                params.nonce, params.gas_price_limit, params.commit_ops, params.verify_ops, params.withdraw_ops
            )
            .execute(self.0.conn())
            .await?;
        }

        Ok(())
    }
}
