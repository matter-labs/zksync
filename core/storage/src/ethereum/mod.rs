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
use self::records::{
    NewETHOperation, NewLastWatchedEthBlockNumber, StorageETHOperation,
    StoredLastWatchedEthBlockNumber,
};
use crate::chain::operations::records::StoredOperation;
use crate::schema::*;
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

    /// Stores the last seen Ethereum block number.
    pub(crate) fn update_last_watched_block_number(
        &self,
        number: &NewLastWatchedEthBlockNumber,
    ) -> QueryResult<()> {
        self.0.conn().transaction(|| {
            diesel::delete(data_restore_last_watched_eth_block::table).execute(self.0.conn())?;
            diesel::insert_into(data_restore_last_watched_eth_block::table)
                .values(number)
                .execute(self.0.conn())?;
            Ok(())
        })
    }

    /// Loads the last seen Ethereum block number.
    pub fn load_last_watched_block_number(&self) -> QueryResult<StoredLastWatchedEthBlockNumber> {
        data_restore_last_watched_eth_block::table.first(self.0.conn())
    }
}
