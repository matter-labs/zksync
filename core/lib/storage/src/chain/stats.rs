use std::cmp::max;
// Built-in deps
use std::time::Instant;
// External imports

// Workspace imports
use zksync_types::{BlockNumber, SequentialTxId};
// Local imports
use crate::{QueryResult, StorageProcessor};

/// Auxiliary schema encapsulating the stats counting logic for the storage tables.
#[derive(Debug)]
pub struct StatsSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> StatsSchema<'a, 'c> {
    /// Returns the amount of blocks that don't have proofs yet.
    pub async fn count_outstanding_proofs(&mut self, after_block: BlockNumber) -> QueryResult<u32> {
        let start = Instant::now();
        let count = sqlx::query!(
            "SELECT COUNT(*) FROM executed_transactions WHERE block_number > $1",
            i64::from(*after_block)
        )
        .fetch_one(self.0.conn())
        .await?
        .count
        .unwrap_or(0);

        metrics::histogram!("sql.chain.stats.count_outstanding_proofs", start.elapsed());
        Ok(count as u32)
    }

    /// Count total transactions after tx_id, and return count and max tx_id.
    /// It allows us to cache count of transactions and make these queries much faster
    pub async fn count_total_transactions(
        &mut self,
        after_tx_id: SequentialTxId,
    ) -> QueryResult<(u32, SequentialTxId)> {
        let after_tx_id = *after_tx_id as i64;
        let start = Instant::now();
        let tx_res = sqlx::query!(
            "SELECT COUNT(*), MAX(sequence_number) FROM executed_transactions 
                 WHERE success = true AND sequence_number > $1",
            after_tx_id
        )
        .fetch_one(self.0.conn())
        .await?;

        let prior_ops_res = sqlx::query!(
            "SELECT COUNT(*), MAX(sequence_number) FROM executed_priority_operations WHERE sequence_number > $1",
            after_tx_id
        )
        .fetch_one(self.0.conn())
        .await?;

        metrics::histogram!("sql.chain.stats.count_total_transactions", start.elapsed());
        Ok((
            (tx_res.count.unwrap_or_default() + prior_ops_res.count.unwrap_or_default()) as u32,
            SequentialTxId(max(
                prior_ops_res.max.unwrap_or(after_tx_id),
                tx_res.max.unwrap_or(after_tx_id),
            ) as u64),
        ))
    }
}
