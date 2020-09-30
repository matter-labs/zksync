// External imports

// Workspace imports
use zksync_types::BlockNumber;
// Local imports
use crate::{QueryResult, StorageProcessor};

/// Auxiliary schema encapsulating the stats counting logic for the storage tables.
#[derive(Debug)]
pub struct StatsSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> StatsSchema<'a, 'c> {
    /// Returns the amount of blocks that don't have proofs yet.
    pub async fn count_outstanding_proofs(&mut self, after_block: BlockNumber) -> QueryResult<u32> {
        let count = sqlx::query!(
            "SELECT COUNT(*) FROM executed_transactions WHERE block_number > $1",
            i64::from(after_block)
        )
        .fetch_one(self.0.conn())
        .await?
        .count
        .unwrap_or(0);

        Ok(count as u32)
    }

    /// Returns the amount of executed transactions (both usual and priority).
    pub async fn count_total_transactions(&mut self) -> QueryResult<u32> {
        let count_tx =
            sqlx::query!("SELECT COUNT(*) FROM executed_transactions WHERE success = true",)
                .fetch_one(self.0.conn())
                .await?
                .count
                .unwrap_or(0);

        let prior_ops = sqlx::query!("SELECT COUNT(*) FROM executed_priority_operations",)
            .fetch_one(self.0.conn())
            .await?
            .count
            .unwrap_or(0);
        Ok((count_tx + prior_ops) as u32)
    }
}
