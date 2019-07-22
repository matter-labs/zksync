use models::plasma::tx::FranklinTx;

use super::ConnectionHolder;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};

use serde_json::Value;

use chrono::NaiveDateTime;

use super::schema::*;
use super::StorageAccount;
use diesel::expression::dsl::count;
use diesel::insert_into;

pub struct Mempool {
    conn: ConnectionHolder,
}

#[derive(Debug, Insertable)]
#[table_name = "mempool"]
struct InsertTx {
    hash: Vec<u8>,
    primary_account: Option<i32>,
    nonce: i64,
    tx: Value,
}

#[derive(Debug, Queryable)]
struct ReadTx {
    hash: Vec<u8>,
    primary_account: Option<i32>,
    nonce: i64,
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
        mempool::table
            .select(count(mempool::primary_account))
            .execute(self.conn())
    }

    pub fn add_tx(&self, tx: &FranklinTx) -> QueryResult<()> {
        insert_into(mempool::table)
            .values(&InsertTx {
                hash: tx.hash(),
                primary_account: tx.account_id().map(|id| id as i32),
                nonce: i64::from(tx.nonce()),
                tx: serde_json::to_value(tx).unwrap(),
            })
            .execute(self.conn())
            .map(drop)
    }

    pub fn get_txs(&self, max_size: usize) -> QueryResult<Vec<FranklinTx>> {
        //TODO use "gaps and islands" sql solution for this.
        let stored_txs: Vec<_> = mempool::table
            .inner_join(accounts::table.on(mempool::primary_account.eq(accounts::id.nullable())))
            .filter(accounts::nonce.eq(mempool::nonce))
            .order(mempool::created_at.asc())
            .limit(max_size as i64)
            .load::<(ReadTx, StorageAccount)>(self.conn())?;

        let new_account_txs: Vec<_> = mempool::table
            .filter(mempool::primary_account.is_null())
            .order(mempool::created_at.asc())
            .limit(max_size as i64)
            .load::<ReadTx>(self.conn())?;

        let mut txs = Vec::new();
        txs.extend(
            stored_txs
                .into_iter()
                .map(|(stored_tx, _)| serde_json::from_value(stored_tx.tx).unwrap()),
        );
        txs.extend(
            new_account_txs
                .into_iter()
                .map(|stored_tx| serde_json::from_value(stored_tx.tx).unwrap()),
        );
        txs.truncate(max_size);
        Ok(txs)
    }
}
