// Built-in deps
use std::{collections::VecDeque, convert::TryFrom, str::FromStr};
// External imports
use diesel::dsl::{insert_into, update};
use diesel::prelude::*;
use num::BigUint;
use web3::types::{H256, U256};
// Workspace imports
use models::{
    ethereum::{ETHOperation, InsertedOperationResponse, OperationType},
    Operation,
};
// Local imports
use self::records::{
    ETHBinding, ETHParams, ETHStats, ETHTxHash, NewETHBinding, NewETHOperation, NewETHTxHash,
    StorageETHOperation,
};
use crate::chain::operations::records::StoredOperation;
use crate::schema::*;
use crate::utils::StoredBigUint;
use crate::StorageProcessor;

pub mod records;

/// Ethereum schema is capable of storing the information about the
/// interaction with the Ethereum blockchain (mainly the list of sent
/// Ethereum transactions).
#[derive(Debug)]
pub struct EthereumSchema<'a>(pub &'a StorageProcessor);

impl<'a> EthereumSchema<'a> {
    /// Loads the list of operations that were not confirmed on Ethereum,
    /// each operation has a list of sent Ethereum transactions.
    pub fn load_unconfirmed_operations(&self) -> QueryResult<VecDeque<ETHOperation>> {
        // Load the operations with the associated Ethereum transactions
        // from the database.
        // Here we obtain a sequence of one-to-one mappings (ETH tx) -> (operation ID).
        // Each Ethereum transaction can have no more than one associated operation, and each
        // operation is associated with exactly one Ethereum transaction. Note that there may
        // be ETH transactions without an operation (e.g. `completeWithdrawals` call), but for
        // every operation always there is an ETH transaction.
        self.0.conn().transaction(|| {
            let raw_ops: Vec<(
                StorageETHOperation,
                Option<ETHBinding>,
                Option<StoredOperation>,
            )> = eth_operations::table
                .left_join(
                    eth_ops_binding::table.on(eth_operations::id.eq(eth_ops_binding::eth_op_id)),
                )
                .left_join(operations::table.on(operations::id.eq(eth_ops_binding::op_id)))
                .filter(eth_operations::confirmed.eq(false))
                .order(eth_operations::id.asc())
                .load(self.0.conn())?;

            // Create a vector for the expected output.
            let mut ops: VecDeque<ETHOperation> = VecDeque::with_capacity(raw_ops.len());

            // Transform the `StoredOperation` to `Operation` and `StoredETHOperation` to `ETHOperation`.
            for (eth_op, _, raw_op) in raw_ops {
                // Load the stored txs hashes ordered by their ID,
                // so the latest added hash will be the last one in the list.
                let eth_tx_hashes: Vec<ETHTxHash> = eth_tx_hashes::table
                    .filter(eth_tx_hashes::eth_op_id.eq(eth_op.id))
                    .order_by(eth_tx_hashes::id.asc())
                    .load(self.0.conn())?;
                assert!(
                    !eth_tx_hashes.is_empty(),
                    "No hashes stored for the Ethereum operation"
                );

                // If there is an operation, convert it to the `Operation` type.
                let op = if let Some(raw_op) = raw_op {
                    Some(raw_op.into_op(self.0)?)
                } else {
                    None
                };

                // Convert the fields into expected format.
                let op_type = OperationType::from_str(eth_op.op_type.as_ref())
                    .expect("Stored operation type must have a valid value");
                let last_used_gas_price =
                    U256::from_str(&eth_op.last_used_gas_price.0.to_string()).unwrap();
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

            Ok(ops)
        })
    }

    /// Loads the operations which were stored in `operations` table, but not
    /// in the `eth_operations`. This method is intended to be used after relaunch
    /// to synchronize `eth_sender` state, as operations are sent to the `eth_sender`
    /// only once.
    pub fn load_unprocessed_operations(&self) -> QueryResult<Vec<Operation>> {
        let raw_ops: Vec<(StoredOperation, Option<ETHBinding>)> =
            self.0.conn().transaction(|| {
                operations::table
                    .left_join(eth_ops_binding::table.on(operations::id.eq(eth_ops_binding::op_id)))
                    .filter(operations::confirmed.eq(false))
                    .order(operations::id.asc())
                    .load(self.0.conn())
            })?;

        let operations: Vec<Operation> = raw_ops
            .into_iter()
            .filter_map(|(raw_op, maybe_binding)| {
                // We are only interested in operations unknown to `eth_operations` table.
                if maybe_binding.is_some() {
                    None
                } else {
                    Some(raw_op.into_op(self.0).expect("Can't convert the operation"))
                }
            })
            .collect();

        Ok(operations)
    }

    /// Stores the sent (but not confirmed yet) Ethereum transaction in the database.
    /// Returns the `ETHOperation` object containing the assigned nonce and operation ID.
    pub fn save_new_eth_tx(
        &self,
        op_type: OperationType,
        op_id: Option<i64>,
        last_deadline_block: i64,
        last_used_gas_price: BigUint,
        raw_tx: Vec<u8>,
    ) -> QueryResult<InsertedOperationResponse> {
        self.0.conn().transaction(|| {
            // It's important to assign nonce within the same db transaction
            // as saving the operation to avoid the state divergence.
            let nonce = self.get_next_nonce()?;

            // Create and insert the operation.
            let operation = NewETHOperation {
                op_type: op_type.to_string(),
                nonce,
                last_deadline_block,
                last_used_gas_price: last_used_gas_price.into(),
                raw_tx,
            };

            let inserted_tx = insert_into(eth_operations::table)
                .values(&operation)
                .returning(eth_operations::id)
                .get_results(self.0.conn())?;
            assert_eq!(
                inserted_tx.len(),
                1,
                "Wrong amount of updated rows (eth_operations)"
            );

            // Obtain the operation ID for the follow-up queried.
            let eth_op_id = inserted_tx[0];

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
                let binding = NewETHBinding { op_id, eth_op_id };

                insert_into(eth_ops_binding::table)
                    .values(&binding)
                    .execute(self.0.conn())?;
            }

            // Update the stored stats.
            self.report_created_operation(op_type)?;

            // Return the assigned ID and nonce.
            let response = InsertedOperationResponse {
                id: eth_op_id,
                nonce: nonce.into(),
            };

            Ok(response)
        })
    }

    /// Retrieves the Ethereum operation ID given the tx hash.
    fn get_eth_op_id(&self, hash: &H256) -> QueryResult<i64> {
        let hash_entry = eth_tx_hashes::table
            .filter(eth_tx_hashes::tx_hash.eq(hash.as_bytes()))
            .first::<ETHTxHash>(self.0.conn())?;

        Ok(hash_entry.eth_op_id)
    }

    /// Adds a tx hash entry associated with some Ethereum operation to the database.
    pub fn add_hash_entry(&self, eth_op_id: i64, hash: &H256) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            // Insert the new hash entry.
            let hash_entry = NewETHTxHash {
                eth_op_id,
                tx_hash: hash.as_bytes().to_vec(),
            };
            let inserted_hashes_rows = insert_into(eth_tx_hashes::table)
                .values(&hash_entry)
                .execute(self.0.conn())?;
            assert_eq!(
                inserted_hashes_rows, 1,
                "Wrong amount of updated rows (eth_tx_hashes)"
            );
            Ok(())
        })
    }

    /// Updates the Ethereum operation by adding a new tx data.
    /// The new deadline block / gas value are placed instead of old values to the main entry.
    pub fn update_eth_tx(
        &self,
        eth_op_id: i64,
        new_deadline_block: i64,
        new_gas_value: BigUint,
    ) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            // Update the stored tx.
            update(eth_operations::table.filter(eth_operations::id.eq(eth_op_id)))
                .set((
                    eth_operations::last_used_gas_price.eq(StoredBigUint(new_gas_value)),
                    eth_operations::last_deadline_block.eq(new_deadline_block),
                ))
                .execute(self.0.conn())?;

            Ok(())
        })
    }

    /// Updates the stats counter with the new operation reported.
    /// This method should be called once **per operation**. It means that if transaction
    /// for some operation was stuck, and another transaction was created for it, this method
    /// **should not** be invoked.
    ///
    /// This method expects the database to be initially prepared with inserting the actual
    /// stats values. Currently the script `db-insert-eth-data.sh` is responsible for that
    /// and it's invoked within `db-reset` subcommand.
    fn report_created_operation(&self, operation_type: OperationType) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            let mut current_stats: ETHParams = eth_parameters::table.first(self.0.conn())?;

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
            update(eth_parameters::table.filter(eth_parameters::id.eq(true)))
                .set((
                    eth_parameters::commit_ops.eq(current_stats.commit_ops),
                    eth_parameters::verify_ops.eq(current_stats.verify_ops),
                    eth_parameters::withdraw_ops.eq(current_stats.withdraw_ops),
                ))
                .execute(self.0.conn())?;

            Ok(())
        })
    }

    /// Updates the stored gas price limit used by GasAdjuster.
    ///
    /// This method expects the database to be initially prepared with inserting the actual
    /// gas limit value. Currently the script `db-insert-eth-data.sh` is responsible for that
    /// and it's invoked within `db-reset` subcommand.
    pub fn update_gas_price_limit(&self, gas_price_limit: U256) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            let gas_price_limit: i64 =
                i64::try_from(gas_price_limit).expect("Can't convert U256 to i64");

            // Update the stored gas price limit.
            update(eth_parameters::table.filter(eth_parameters::id.eq(true)))
                .set(eth_parameters::gas_price_limit.eq(gas_price_limit))
                .execute(self.0.conn())?;

            Ok(())
        })
    }

    pub fn load_gas_price_limit(&self) -> QueryResult<U256> {
        let params: ETHParams = eth_parameters::table.first::<ETHParams>(self.0.conn())?;

        let gas_price_limit =
            U256::try_from(params.gas_price_limit).expect("Negative gas limit value stored in DB");

        Ok(gas_price_limit)
    }

    /// Loads the stored Ethereum operations stats.
    pub fn load_stats(&self) -> QueryResult<ETHStats> {
        eth_parameters::table
            .first::<ETHParams>(self.0.conn())
            .map(ETHStats::from)
    }

    /// Marks the stored Ethereum transaction as confirmed (and thus the associated `Operation`
    /// is marked as confirmed as well).
    pub fn confirm_eth_tx(&self, hash: &H256) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            let eth_op_id = self.get_eth_op_id(hash)?;

            // Set the `confirmed` and `final_hash` field of the entry.
            let updated: Vec<i64> =
                update(eth_operations::table.filter(eth_operations::id.eq(eth_op_id)))
                    .set((
                        eth_operations::confirmed.eq(true),
                        eth_operations::final_hash.eq(Some(hash.as_bytes().to_vec())),
                    ))
                    .returning(eth_operations::id)
                    .get_results(self.0.conn())?;

            assert_eq!(
                updated.len(),
                1,
                "Unexpected amount of operations were confirmed"
            );

            let eth_op_id = updated[0];

            let binding: Option<ETHBinding> = eth_ops_binding::table
                .filter(eth_ops_binding::eth_op_id.eq(eth_op_id))
                .first::<ETHBinding>(self.0.conn())
                .optional()?;

            // If there is a ZKSync operation, mark it as confirmed as well.
            if let Some(binding) = binding {
                let op = operations::table
                    .filter(operations::id.eq(binding.op_id))
                    .first::<StoredOperation>(self.0.conn())?;

                update(operations::table.filter(operations::id.eq(op.id)))
                    .set(operations::confirmed.eq(true))
                    .execute(self.0.conn())
                    .map(drop)?;
            }

            Ok(())
        })
    }

    /// Obtains the next nonce to use and updates the corresponding entry in the database
    /// for the next invocation.
    ///
    /// This method expects the database to be initially prepared with inserting the actual
    /// nonce value. Currently the script `db-insert-eth-data.sh` is responsible for that
    /// and it's invoked within `db-reset` subcommand.
    pub(crate) fn get_next_nonce(&self) -> QueryResult<i64> {
        let old_nonce: ETHParams = eth_parameters::table.first(self.0.conn())?;

        let new_nonce_value = old_nonce.nonce + 1;

        update(eth_parameters::table.filter(eth_parameters::id.eq(true)))
            .set(eth_parameters::nonce.eq(new_nonce_value))
            .execute(self.0.conn())?;

        let old_nonce_value = old_nonce.nonce;

        Ok(old_nonce_value)
    }

    /// Method that internally initializes the `eth_parameters` table.
    /// Since in db tests the database is empty, we must provide a possibility
    /// to initialize required db fields.
    #[cfg(test)]
    pub fn initialize_eth_data(&self) -> QueryResult<()> {
        #[derive(Debug, Insertable)]
        #[table_name = "eth_parameters"]
        pub struct NewETHParams {
            pub nonce: i64,
            pub gas_price_limit: i64,
            pub commit_ops: i64,
            pub verify_ops: i64,
            pub withdraw_ops: i64,
        }

        let old_params: Option<ETHParams> =
            eth_parameters::table.first(self.0.conn()).optional()?;

        if old_params.is_none() {
            let params = NewETHParams {
                nonce: 0,
                gas_price_limit: 400 * 10e9 as i64,
                commit_ops: 0,
                verify_ops: 0,
                withdraw_ops: 0,
            };

            insert_into(eth_parameters::table)
                .values(&params)
                .execute(self.0.conn())?;
        }

        Ok(())
    }
}
