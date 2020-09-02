// Built-in deps
use std::collections::VecDeque;
// External imports
use diesel::dsl::max;
use diesel::prelude::*;
use itertools::Itertools;
// Workspace imports
use models::node::{mempool::SignedTxVariant, tx::TxHash, SignedFranklinTx};
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
    pub fn load_txs(&self) -> Result<VecDeque<SignedTxVariant>, failure::Error> {
        // Load the transactions from mempool along with corresponding batch IDs.
        let txs: Vec<MempoolTx> = mempool_txs::table.load(self.0.conn())?;

        let mut prev_batch_id = txs.first().map(|tx| tx.batch_id).flatten();

        let grouped_txs = txs.into_iter().group_by(|tx| {
            prev_batch_id = tx.batch_id;

            prev_batch_id
        });

        let mut txs = VecDeque::new();

        for (batch_id, group) in grouped_txs.into_iter() {
            let deserialized_txs: Vec<SignedFranklinTx> = group
                .map(|tx_object| -> Result<SignedFranklinTx, failure::Error> {
                    let tx = serde_json::from_value(tx_object.tx)?;
                    let sign_data = match tx_object.eth_sign_data {
                        None => None,
                        Some(sign_data_value) => serde_json::from_value(sign_data_value)?,
                    };

                    Ok(SignedFranklinTx {
                        tx,
                        eth_sign_data: sign_data,
                    })
                })
                .collect::<Result<Vec<SignedFranklinTx>, failure::Error>>()?;

            match batch_id {
                Some(_) => {
                    // Group of batched transactions.
                    let variant = SignedTxVariant::from(deserialized_txs);
                    txs.push_back(variant);
                }
                None => {
                    // Group of non-batched transactions.
                    let mut variants = deserialized_txs
                        .into_iter()
                        .map(SignedTxVariant::from)
                        .collect();
                    txs.append(&mut variants);
                }
            }
        }

        Ok(txs)
    }

    /// Adds a new transactions batch to the mempool schema.
    pub fn insert_batch(&self, txs: &[SignedFranklinTx]) -> Result<(), failure::Error> {
        if txs.is_empty() {
            failure::bail!("Cannot insert an empty batch");
        }

        self.0.transaction(|| {
            // Batch ID is set to the maximum transaction ID in the table. It is guaranteed to be unique,
            // since as long as batch exists, the maximum ID will be greater than the batch ID (as we've inserted
            // more transactions).
            let batch_id = mempool_txs::table
                .select(max(mempool_txs::id))
                .first::<Option<i64>>(self.0.conn())?
                .unwrap_or(0);

            let new_transactions: Vec<_> = txs
                .iter()
                .map(|tx_data| {
                    let tx_hash = hex::encode(tx_data.hash().as_ref());
                    let tx = serde_json::to_value(tx_data)
                        .expect("Unserializable TX provided to the database");

                    NewMempoolTx {
                        tx_hash,
                        tx,
                        created_at: chrono::Utc::now(),
                        eth_sign_data: tx_data.eth_sign_data.as_ref().map(|sd| {
                            serde_json::to_value(sd).expect("failed to encode EthSignData")
                        }),
                        batch_id: Some(batch_id),
                    }
                })
                .collect();

            diesel::insert_into(mempool_txs::table)
                .values(new_transactions)
                .execute(self.0.conn())?;

            Ok(())
        })
    }

    /// Adds a new transaction to the mempool schema.
    pub fn insert_tx(&self, tx_data: &SignedFranklinTx) -> Result<(), failure::Error> {
        let tx_hash = hex::encode(tx_data.tx.hash().as_ref());
        let tx = serde_json::to_value(&tx_data.tx)?;
        let batch_id = None;

        let db_entry = NewMempoolTx {
            tx_hash,
            tx,
            created_at: chrono::Utc::now(),
            eth_sign_data: tx_data
                .eth_sign_data
                .as_ref()
                .map(|sd| serde_json::to_value(sd).expect("failed to encode EthSignData")),
            batch_id,
        };

        diesel::insert_into(mempool_txs::table)
            .values(db_entry)
            .execute(self.0.conn())?;

        Ok(())
    }

    pub fn remove_tx(&self, tx: &[u8]) -> QueryResult<()> {
        let tx_hash = hex::encode(tx);

        diesel::delete(mempool_txs::table.filter(mempool_txs::tx_hash.eq(&tx_hash)))
            .execute(self.0.conn())?;

        Ok(())
    }

    pub fn remove_txs(&self, txs: &[TxHash]) -> Result<(), failure::Error> {
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
                SignedTxVariant::Tx(tx) => {
                    let tx_hash = tx.hash();
                    self.0
                        .chain()
                        .operations_ext_schema()
                        .get_tx_by_hash(tx_hash.as_ref())
                        .expect("DB issue while restoring the mempool state")
                        .is_some()
                }
                SignedTxVariant::Batch(batch) => {
                    // We assume that for batch one executed transaction <=> all the transactions are executed.
                    let tx_hash = batch.0[0].hash();
                    self.0
                        .chain()
                        .operations_ext_schema()
                        .get_tx_by_hash(tx_hash.as_ref())
                        .expect("DB issue while restoring the mempool state")
                        .is_some()
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
