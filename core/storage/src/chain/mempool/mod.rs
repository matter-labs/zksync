// Built-in deps
// External imports
use diesel::prelude::*;
// Workspace imports
// Local imports
use crate::schema::*;
use crate::{chain::operations_ext::records::InsertTx, StorageProcessor};

pub(crate) struct MempoolSchema<'a>(pub &'a StorageProcessor);

impl<'a> MempoolSchema<'a> {
    pub fn insert_tx(&self, tx: InsertTx) -> QueryResult<()> {
        diesel::insert_into(mempool::table)
            .values(tx)
            .on_conflict_do_nothing()
            .execute(self.0.conn())?;
        Ok(())
    }
}
