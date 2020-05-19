// Built-in deps
use std::collections::VecDeque;
// External imports
use diesel::prelude::*;
// Workspace imports
use models::node::FranklinTx;
// Local imports
use self::records::MempoolTx;
use crate::{schema::*, StorageProcessor};

pub mod records;

/// Schema for TODO
#[derive(Debug)]
pub struct MempoolSchema<'a>(pub &'a StorageProcessor);

impl<'a> MempoolSchema<'a> {
    /// Loads all the transactions stored in the mempool schema.
    pub fn load_txs(&self) -> Result<VecDeque<FranklinTx>, failure::Error> {
        let txs: Vec<MempoolTx> = mempool_txs::table.load(self.0.conn())?;

        let txs = txs
            .into_iter()
            .map(|tx_object| serde_json::from_value(tx_object.tx))
            .collect::<Result<VecDeque<FranklinTx>, _>>()?;
        Ok(txs)
    }

    /// Adds a new transaction to the mempool schema.
    pub fn insert_tx(&self, tx_data: &FranklinTx) -> Result<(), failure::Error> {
        let tx_hash = tx_data.hash().to_string();
        let tx = serde_json::to_value(tx_data)?;

        let db_entry = MempoolTx { tx_hash, tx };

        diesel::insert_into(mempool_txs::table)
            .values(db_entry)
            .execute(self.0.conn())?;

        Ok(())
    }

    pub fn remove_txs(&self, txs: &[FranklinTx]) -> Result<(), failure::Error> {
        let tx_hashes: Vec<_> = txs.iter().map(|tx| tx.hash().to_string()).collect();

        diesel::delete(mempool_txs::table.filter(mempool_txs::tx_hash.eq_any(&tx_hashes)))
            .execute(self.0.conn())?;

        Ok(())
    }

    /// Removes transactions that are already committed.
    /// Though it's unlikely that mempool schema will ever contain a committed
    /// transaction, it's better to ensure that we won't process the same transaction
    /// again. One possible scenario for having already-processed txs in the database
    /// is a failure of `remove_txs` method, which won't cause a panic on server, but will
    /// left txs in the database.
    ///
    /// This method is expected to be initially invoked on the server start, and then
    /// invoked periodically with a big interval (to prevent possible database bloating).
    pub fn collect_garbage(&self) -> Result<(), failure::Error> {
        let mut txs_to_remove: Vec<_> = self.load_txs()?.into_iter().collect();
        txs_to_remove.retain(|tx| {
            let tx_hash = tx.hash();
            self.0
                .chain()
                .operations_ext_schema()
                .get_tx_by_hash(tx_hash.as_ref())
                .expect("DB issue while restoring the mempool state")
                .is_some()
        });

        self.remove_txs(&txs_to_remove)?;

        Ok(())
    }
}
