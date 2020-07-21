// Built-in deps
use std::collections::VecDeque;
// External imports
use diesel::prelude::*;
// Workspace imports
use models::node::{mempool::TxVariant, tx::TxHash, FranklinTx};
// Local imports
use self::records::{MempoolTx, NewMempoolTx};
use crate::{schema::*, StorageProcessor};

pub mod records;

/// Schema for persisting transactions awaiting for the execution.
///
/// This schema holds the transactions that are received by the `mempool` module, but not yet have
/// been included into some block. It is required to store these transactions in the database, so
/// in case of the unexpected server reboot sent transactions won't disappear, and will be executed
/// as if the server haven't been relaunched.
#[derive(Debug)]
pub struct MempoolSchema<'a>(pub &'a StorageProcessor);

impl<'a> MempoolSchema<'a> {
    /// Loads all the transactions stored in the mempool schema.
    pub fn load_txs(&self) -> Result<VecDeque<TxVariant>, failure::Error> {
        let txs: Vec<MempoolTx> = mempool_txs::table.load(self.0.conn())?;

        let txs = txs
            .into_iter()
            .map(|tx_object| serde_json::from_value(tx_object.tx))
            .map(|tx: Result<FranklinTx, _>| tx.map(TxVariant::from))
            .collect::<Result<VecDeque<TxVariant>, _>>()?;
        Ok(txs)
    }

    /// Adds a new transaction to the mempool schema.
    pub fn insert_tx(&self, tx_data: &FranklinTx) -> Result<(), failure::Error> {
        let tx_hash = hex::encode(tx_data.hash().as_ref());
        let tx = serde_json::to_value(tx_data)?;

        let db_entry = NewMempoolTx { tx_hash, tx };

        diesel::insert_into(mempool_txs::table)
            .values(db_entry)
            .execute(self.0.conn())?;

        Ok(())
    }

    pub fn remove_tx(&self, tx: &[u8]) -> QueryResult<()> {
        let tx_hash = hex::encode(tx);

        diesel::delete(mempool_txs::table.filter(mempool_txs::tx_hash.eq(&tx_hash)))
            .execute(self.0.conn())?;

        // TODO: Check if there is a corresponding batch for the tx, and remove it as well if necessary.

        Ok(())
    }

    fn remove_txs(&self, txs: &[TxHash]) -> Result<(), failure::Error> {
        let tx_hashes: Vec<_> = txs.iter().map(hex::encode).collect();

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
            match tx {
                TxVariant::Tx(tx) => {
                    let tx_hash = tx.hash();
                    self.0
                        .chain()
                        .operations_ext_schema()
                        .get_tx_by_hash(tx_hash.as_ref())
                        .expect("DB issue while restoring the mempool state")
                        .is_some()
                }
                TxVariant::Batch(_batch) => {
                    // TODO
                    unimplemented!()
                }
            }
        });

        let tx_hashes: Vec<_> = txs_to_remove
            .into_iter()
            .map(|tx| tx.hashes())
            .flatten()
            .collect();

        self.remove_txs(&tx_hashes)?;

        Ok(())
    }
}
