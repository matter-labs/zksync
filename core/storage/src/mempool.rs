use models::plasma::tx::TransferTx;

use super::ConnectionHolder;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};

use serde_json::Value;

use chrono::NaiveDateTime;

use super::schema::*;
use diesel::insert_into;

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

    pub fn add_tx(&self, tx: &TransferTx) -> QueryResult<()> {
        unimplemented!()
    }

    pub fn get_txs(&self, max_size: usize) -> QueryResult<Vec<TransferTx>> {
        unimplemented!()
    }

    pub fn remove_txs(&self, ids: &[i32]) -> QueryResult<()> {
        unimplemented!()
    }
}
