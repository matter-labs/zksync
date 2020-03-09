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
use self::records::{NewETHOperation, StorageETHOperation, StoredLastWatchedEthBlockNumber};
use crate::interfaces::operations::records::StoredOperation;
use crate::schema::*;
use crate::StorageProcessor;

pub mod records;

pub struct EthereumSchema<'a>(pub &'a StorageProcessor);

impl<'a> EthereumSchema<'a> {
    pub fn load_unconfirmed_operations(
        &self,
        // TODO: move Eth transaction state to models and add it here
    ) -> QueryResult<Vec<(Operation, Vec<StorageETHOperation>)>> {
        self.0.conn().transaction(|| {
            let ops: Vec<_> = operations::table
                .left_join(eth_operations::table.on(eth_operations::op_id.eq(operations::id)))
                .filter(operations::confirmed.eq(false))
                .order(operations::id.asc())
                .load::<(StoredOperation, Option<StorageETHOperation>)>(self.0.conn())?;

            let mut ops = ops
                .into_iter()
                .map(|(o, e)| o.into_op(self.0).map(|o| (o, e)))
                .collect::<QueryResult<Vec<_>>>()?;
            ops.sort_by_key(|(o, _)| o.id.unwrap()); // operations from db MUST have and id.

            Ok(ops
                .into_iter()
                .group_by(|(o, _)| o.id.unwrap())
                .into_iter()
                .map(|(_op_id, group_iter)| {
                    let fold_result = group_iter.fold(
                        (None, Vec::new()),
                        |(mut accum_op, mut accum_eth_ops): (Option<Operation>, _),
                         (op, eth_op)| {
                            if let Some(accum_op) = accum_op.as_ref() {
                                assert_eq!(accum_op.id, op.id);
                            } else {
                                accum_op = Some(op);
                            }
                            if let Some(eth_op) = eth_op {
                                accum_eth_ops.push(eth_op);
                            }

                            (accum_op, accum_eth_ops)
                        },
                    );
                    (fold_result.0.unwrap(), fold_result.1)
                })
                .collect())
        })
    }

    pub fn load_sent_unconfirmed_ops(
        &self,
    ) -> QueryResult<Vec<(Operation, Vec<StorageETHOperation>)>> {
        self.0.conn().transaction(|| {
            let ops: Vec<_> = operations::table
                .filter(eth_operations::confirmed.eq(false))
                .inner_join(eth_operations::table.on(eth_operations::op_id.eq(operations::id)))
                .order(operations::id.asc())
                .load::<(StoredOperation, StorageETHOperation)>(self.0.conn())?;
            let mut ops_with_eth_actions = Vec::new();
            for (op, eth_op) in ops.into_iter() {
                ops_with_eth_actions.push((op.into_op(self.0)?, eth_op));
            }
            Ok(ops_with_eth_actions
                .into_iter()
                .group_by(|(o, _)| o.id.unwrap())
                .into_iter()
                .map(|(_op_id, group_iter)| {
                    let fold_result = group_iter.fold(
                        (None, Vec::new()),
                        |(mut accum_op, mut accum_eth_ops): (Option<Operation>, _),
                         (op, eth_op)| {
                            if let Some(accum_op) = accum_op.as_ref() {
                                assert_eq!(accum_op.id, op.id);
                            } else {
                                accum_op = Some(op);
                            }
                            accum_eth_ops.push(eth_op);

                            (accum_op, accum_eth_ops)
                        },
                    );
                    (fold_result.0.unwrap(), fold_result.1)
                })
                .collect())
        })
    }

    pub fn save_operation_eth_tx(
        &self,
        op_id: i64,
        hash: H256,
        deadline_block: u64,
        nonce: u32,
        gas_price: BigDecimal,
        raw_tx: Vec<u8>,
    ) -> QueryResult<()> {
        insert_into(eth_operations::table)
            .values(&NewETHOperation {
                op_id,
                nonce: i64::from(nonce),
                deadline_block: deadline_block as i64,
                gas_price,
                tx_hash: hash.as_bytes().to_vec(),
                raw_tx,
            })
            .execute(self.0.conn())
            .map(drop)
    }

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

    pub fn load_last_watched_block_number(&self) -> QueryResult<StoredLastWatchedEthBlockNumber> {
        data_restore_last_watched_eth_block::table.first(self.0.conn())
    }
}
