use models::plasma::tx::TransferTx;

use super::ConnectionHolder;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};

use serde_json::Value;

use chrono::NaiveDateTime;

use super::schema::*;
use diesel::insert_into;
use diesel::expression::dsl::count;

pub struct Mempool {
    conn: ConnectionHolder,
}

#[derive(Debug, Insertable)]
#[table_name = "mempool"]
struct InsertTx {
    tx: Value,
}

#[derive(Debug, Queryable)]
struct ReadTx {
    id: i32,
    tx: Value,
    created_at: NaiveDateTime,
}

impl Mempool {
    pub(crate) fn from_db_connect_pool(
        pool: PooledConnection<ConnectionManager<PgConnection>>,
    ) -> Self {
        Self {
            conn: ConnectionHolder::Pooled(pool),
        }
    }

    fn conn(&self) -> &PgConnection {
        match self.conn {
            ConnectionHolder::Pooled(ref conn) => conn,
            ConnectionHolder::Direct(ref conn) => conn,
        }
    }

    pub fn get_size(&self) -> QueryResult<usize> {
        mempool::table.select(count(mempool::id)).execute(self.conn())
    }

    pub fn add_tx(&self, tx: &TransferTx) -> QueryResult<()> {
        insert_into(mempool::table)
            .values(&InsertTx {
                tx: serde_json::to_value(tx).unwrap(),
            })
            .execute(self.conn())
            .map(drop)
    }

    pub fn get_txs(&self, max_size: usize) -> QueryResult<Vec<(i32, TransferTx)>> {
        let stored_txs: Vec<ReadTx> = mempool::table
            .order(mempool::created_at.asc())
            .limit(max_size as i64)
            .load(self.conn())?;

        Ok(stored_txs
            .into_iter()
            .map(|stored_tx| (stored_tx.id, serde_json::from_value(stored_tx.tx).unwrap()))
            .collect())
    }

    pub fn remove_txs(&self, ids: &[i32]) -> QueryResult<()> {
        diesel::delete(mempool::table.filter(mempool::id.eq_any(ids)))
            .execute(self.conn())
            .map(drop)
    }
}
