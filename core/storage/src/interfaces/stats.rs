// External imports

use diesel::dsl::count_star;
use diesel::prelude::*;
// Workspace imports
use models::node::BlockNumber;
// Local imports
use crate::schema::*;
use crate::StorageProcessor;

pub trait StatsInterface {
    fn count_outstanding_proofs(&self, after_block: BlockNumber) -> QueryResult<u32>;
    fn count_total_transactions(&self) -> QueryResult<u32>;
}

impl StatsInterface for StorageProcessor {
    fn count_outstanding_proofs(&self, after_block: BlockNumber) -> QueryResult<u32> {
        use crate::schema::executed_transactions::dsl::*;
        let count: i64 = executed_transactions
            .filter(block_number.gt(i64::from(after_block)))
            .select(count_star())
            .first(self.conn())?;
        Ok(count as u32)
    }

    fn count_total_transactions(&self) -> QueryResult<u32> {
        let count_tx: i64 = executed_transactions::table
            .filter(executed_transactions::success.eq(true))
            .select(count_star())
            .first(self.conn())?;
        let prior_ops: i64 = executed_priority_operations::table
            .select(count_star())
            .first(self.conn())?;
        Ok((count_tx + prior_ops) as u32)
    }
}
