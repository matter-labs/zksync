// Built-in deps
// External imports
use bigdecimal::BigDecimal;
use diesel::dsl::{insert_into, update};
use diesel::prelude::*;
use web3::types::H256;
// Workspace imports
use models::Operation;
// Local imports
use self::records::{
    ETHBinding, ETHNonce, ETHStats, NewETHBinding, NewETHOperation, StorageETHOperation,
};
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

impl OperationType {
    pub fn to_string(&self) -> String {
        match self {
            Self::Commit => "commit".into(),
            Self::Verify => "verify".into(),
            Self::Withdraw => "withdraw".into(),
        }
    }
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
    ) -> QueryResult<Vec<(StorageETHOperation, Option<Operation>)>> {
        // Load the operations with the associated Ethereum transactions
        // from the database.
        // Here we obtain a sequence of one-to-one mappings (ETH tx) -> (operation ID).
        // Each Ethereum transaction can have no more than one associated operation, and each
        // operation is associated with exactly one Ethereum transaction. Note that there may
        // be ETH transactions without an operation (e.g. `completeWithdrawals` call), but for
        // every operation always there is an ETH transaction.
        let raw_ops: Vec<(
            StorageETHOperation,
            Option<ETHBinding>,
            Option<StoredOperation>,
        )> = self.0.conn().transaction(|| {
            eth_operations::table
                .left_join(
                    eth_ops_binding::table.on(eth_operations::id.eq(eth_ops_binding::eth_op_id)),
                )
                .left_join(operations::table.on(operations::id.eq(eth_ops_binding::op_id)))
                .filter(eth_operations::confirmed.eq(false))
                .order(eth_operations::id.asc())
                .load(self.0.conn())
        })?;

        // Create a vector for the expected output.
        let mut ops: Vec<(StorageETHOperation, Option<Operation>)> =
            Vec::with_capacity(raw_ops.len());

        // Transform the `StoredOperation` to `Operation`.
        for (eth_op, _, raw_op) in raw_ops {
            let op = if let Some(raw_op) = raw_op {
                Some(raw_op.into_op(self.0)?)
            } else {
                None
            };

            ops.push((eth_op, op));
        }

        Ok(ops)
    }

    /// Stores the sent (but not confirmed yet) Ethereum transaction in the database.
    pub fn save_new_eth_tx(
        &self,
        op_type: OperationType,
        op_id: Option<i64>,
        hash: H256,
        deadline_block: u64,
        nonce: u32,
        gas_price: BigDecimal,
        raw_tx: Vec<u8>,
    ) -> QueryResult<()> {
        let operation = NewETHOperation {
            op_type: op_type.to_string(),
            nonce: i64::from(nonce),
            deadline_block: deadline_block as i64,
            last_used_gas_price: gas_price,
            tx_hash: hash.as_bytes().to_vec(),
            raw_tx,
        };

        self.0.conn().transaction(|| {
            let inserted = insert_into(eth_operations::table)
                .values(&operation)
                .returning(eth_operations::id)
                .get_results(self.0.conn())?;
            assert_eq!(inserted.len(), 1, "Wrong amount of updated rows");

            let eth_op_id = inserted[0];
            if let Some(op_id) = op_id {
                // If the operation ID was provided, we should also insert a binding entry.
                let binding = NewETHBinding { op_id, eth_op_id };

                insert_into(eth_ops_binding::table)
                    .values(&binding)
                    .execute(self.0.conn())?;
            }

            self.report_created_operation(op_type)?;

            Ok(())
        })
    }

    /// Changes the last used gas for a transaction. Since for every sent transaction the gas
    /// is the only field changed, it makes no sense to duplicate many alike transactions for each
    /// operation. Instead we enforce using exactly one tx for each operation and store only the last
    /// used gas value (to increment later if we'll need to send the tx again).
    pub fn update_eth_tx_gas(&self, hash: &H256, new_gas_value: BigDecimal) -> QueryResult<()> {
        update(eth_operations::table.filter(eth_operations::tx_hash.eq(hash.as_bytes())))
            .set(eth_operations::last_used_gas_price.eq(new_gas_value))
            .execute(self.0.conn())?;

        Ok(())
    }

    /// Updates the stats counter with the new operation reported.
    /// This method should be called once **per operation**. It means that if transaction
    /// for some operation was stuck, and another transaction was created for it, this method
    /// **should not** be invoked.
    ///
    /// This method expects the database to be initially prepared with inserting the actual
    /// nonce value. Currently the script `db-insert-eth-data.sh` is responsible for that
    /// and it's invoked within `db-reset` subcommand.
    fn report_created_operation(&self, operation_type: OperationType) -> QueryResult<()> {
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
            let updated: Vec<i64> =
                update(eth_operations::table.filter(eth_operations::tx_hash.eq(hash.as_bytes())))
                    .set(eth_operations::confirmed.eq(true))
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
