// Built-in deps
// External imports
use bigdecimal::BigDecimal;
use diesel::dsl::{insert_into, update};
use diesel::prelude::*;
use itertools::Itertools;
use web3::types::H256;
// Workspace imports
use models::Operation;
// Local imports
use self::records::{ETHNonce, ETHStats, NewETHOperation, StorageETHOperation};
use crate::chain::operations::records::StoredOperation;
use crate::schema::*;
use crate::StorageProcessor;

pub mod records;

#[derive(Debug, Clone, Copy)]
pub enum OperationType {
    Commit,
    Verify,
    Withdraw,
}

/// Ethereum schema is capable of storing the information about the
/// interaction with the Ethereum blockchain (mainly the list of sent
/// Ethereum transactions).
#[derive(Debug)]
pub struct EthereumSchema<'a>(pub &'a StorageProcessor);

impl<'a> EthereumSchema<'a> {
    /// Loads the list of operations that were not confirmed on Ethereum,
    /// each operation has a list of sent Ethereum transactions.
    pub fn load_unconfirmed_operations(
        &self,
        // TODO: move Eth transaction state to models and add it here
    ) -> QueryResult<Vec<(Operation, Vec<StorageETHOperation>)>> {
        // Load the operations with the associated Ethereum transactions
        // from the database.
        // Here we obtain a sequence of one-to-one mappings (operation ID) -> (ETH operation).
        // This means that operation ID may be encountered multiple times (if there was more than
        // one transaction sent).
        let ops: Vec<(StoredOperation, Option<StorageETHOperation>)> =
            self.0.conn().transaction(|| {
                operations::table
                    .left_join(eth_operations::table.on(eth_operations::op_id.eq(operations::id)))
                    .filter(operations::confirmed.eq(false))
                    .order(operations::id.asc())
                    .load(self.0.conn())
            })?;

        // Transform the `StoredOperation` to `Operation`.
        let mut ops = ops
            .into_iter()
            .map(|(op, eth_ops)| op.into_op(self.0).map(|op| (op, eth_ops)))
            .collect::<QueryResult<Vec<_>>>()?;

        // Sort the operations and group them by key, so we will obtain the groups
        // of Ethereum operations mapped to the operations as a many-to-one mapping.
        ops.sort_by_key(|(op, _)| op.id.expect("Operations in the db MUST have and id"));
        let grouped_operations = ops.into_iter().group_by(|(o, _)| o.id.unwrap());

        // Now go through the groups and collect all the Ethereum transactions to the vectors
        // associated with a certain `Operation`.
        let result = grouped_operations
            .into_iter()
            .map(|(_, group_iter)| {
                // In this fold we have two accumulators:
                // - operation (initialized at the first step, then just checked to be the same).
                // - list of ETH txs (appended on each step).
                let fold_result = group_iter.fold(
                    (None, Vec::new()),
                    |(mut accum_op, mut accum_eth_ops): (Option<Operation>, _), (op, eth_op)| {
                        // Ensure that the grouping was done right and the operation is the same
                        // across the group.
                        assert_eq!(accum_op.get_or_insert_with(|| op.clone()).id, op.id);

                        // Add the Ethereum operation to the list.
                        if let Some(eth_op) = eth_op {
                            accum_eth_ops.push(eth_op);
                        }

                        (accum_op, accum_eth_ops)
                    },
                );
                (fold_result.0.unwrap(), fold_result.1)
            })
            .collect();

        Ok(result)
    }

    /// Stores the sent (but not confirmed yet) Ethereum transaction in the database.
    pub fn save_operation_eth_tx(
        &self,
        op_id: i64,
        hash: H256,
        deadline_block: u64,
        nonce: u32,
        gas_price: BigDecimal,
        raw_tx: Vec<u8>,
    ) -> QueryResult<()> {
        let operation = NewETHOperation {
            op_id,
            nonce: i64::from(nonce),
            deadline_block: deadline_block as i64,
            gas_price,
            tx_hash: hash.as_bytes().to_vec(),
            raw_tx,
        };

        insert_into(eth_operations::table)
            .values(&operation)
            .execute(self.0.conn())
            .map(drop)
    }

    /// Updates the stats counter with the new operation reported.
    /// This method should be called once **per operation**. It means that if transaction
    /// for some operation was stuck, and another transaction was created for it, this method
    /// **should not** be invoked.
    ///
    /// This method expects the database to be initially prepared with inserting the actual
    /// nonce value. Currently the script `db-insert-eth-data.sh` is responsible for that
    /// and it's invoked within `db-reset` subcommand.
    pub fn report_created_operation(&self, operation_type: OperationType) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            let mut current_stats: ETHStats = eth_stats::table.first(self.0.conn())?;

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
            update(eth_stats::table.filter(eth_stats::id.eq(true)))
                .set((
                    eth_stats::commit_ops.eq(current_stats.commit_ops),
                    eth_stats::verify_ops.eq(current_stats.verify_ops),
                    eth_stats::withdraw_ops.eq(current_stats.withdraw_ops),
                ))
                .execute(self.0.conn())?;

            Ok(())
        })
    }

    /// Loads the stored Ethereum operations stats.
    pub fn load_stats(&self) -> QueryResult<ETHStats> {
        eth_stats::table.first(self.0.conn())
    }

    /// Marks the stored Ethereum transaction as confirmed (and thus the associated `Operation`
    /// is marked as confirmed as well).
    pub fn confirm_eth_tx(&self, hash: &H256) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            update(eth_operations::table.filter(eth_operations::tx_hash.eq(hash.as_bytes())))
                .set(eth_operations::confirmed.eq(true))
                .execute(self.0.conn())
                .map(drop)?;
            let (op, _) = operations::table
                .inner_join(eth_operations::table.on(eth_operations::op_id.eq(operations::id)))
                .filter(eth_operations::tx_hash.eq(hash.as_bytes()))
                .first::<(StoredOperation, StorageETHOperation)>(self.0.conn())?;

            update(operations::table.filter(operations::id.eq(op.id)))
                .set(operations::confirmed.eq(true))
                .execute(self.0.conn())
                .map(drop)
        })
    }

    /// Obtains the next nonce to use and updates the corresponding entry in the database
    /// for the next invocation.
    ///
    /// This method expects the database to be initially prepared with inserting the actual
    /// nonce value. Currently the script `db-insert-eth-data.sh` is responsible for that
    /// and it's invoked within `db-reset` subcommand.
    pub fn get_next_nonce(&self) -> QueryResult<i64> {
        let old_nonce: ETHNonce = eth_nonce::table.first(self.0.conn())?;

        let new_nonce_value = old_nonce.nonce + 1;

        update(eth_nonce::table.filter(eth_nonce::id.eq(true)))
            .set(eth_nonce::nonce.eq(new_nonce_value))
            .execute(self.0.conn())?;

        let old_nonce_value = old_nonce.nonce;

        Ok(old_nonce_value)
    }

    /// Method that internally initializes the `eth_nonce` and `eth_stats` tables.
    /// Since in db tests the database is empty, we must provide a possibility
    /// to initialize required db fields.
    #[cfg(test)]
    pub fn initialize_eth_data(&self) -> QueryResult<()> {
        #[derive(Debug, Insertable)]
        #[table_name = "eth_nonce"]
        pub struct NewETHNonce {
            pub nonce: i64,
        }

        #[derive(Debug, Insertable)]
        #[table_name = "eth_stats"]
        pub struct NewETHStats {
            pub commit_ops: i64,
            pub verify_ops: i64,
            pub withdraw_ops: i64,
        }

        let old_nonce: Option<ETHNonce> = eth_nonce::table.first(self.0.conn()).optional()?;

        if old_nonce.is_none() {
            // There is no nonce, we have to insert it manually.
            let nonce = NewETHNonce { nonce: 0 };

            insert_into(eth_nonce::table)
                .values(&nonce)
                .execute(self.0.conn())?;
        }

        let old_stats: Option<ETHStats> = eth_stats::table.first(self.0.conn()).optional()?;

        if old_stats.is_none() {
            let stats = NewETHStats {
                commit_ops: 0,
                verify_ops: 0,
                withdraw_ops: 0,
            };

            insert_into(eth_stats::table)
                .values(&stats)
                .execute(self.0.conn())?;
        }

        Ok(())
    }
}
