use models::node::tx::FranklinTx;
use models::node::AccountAddress;

use super::ConnectionHolder;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};

use serde_json::Value;

use chrono::NaiveDateTime;

use super::schema::*;
use super::{StorageAccount, StoredExecutedTransaction};
use diesel::expression::dsl::count;
use diesel::insert_into;

pub struct Mempool {
    conn: ConnectionHolder,
}

#[derive(Debug, Insertable)]
#[table_name = "mempool"]
struct InsertTx {
    hash: Vec<u8>,
    primary_account_address: Vec<u8>,
    nonce: i64,
    tx: Value,
}

#[derive(Debug, Queryable)]
struct ReadTx {
    hash: Vec<u8>,
    primary_account_address: Vec<u8>,
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
            .select(count(mempool::primary_account_address))
            .execute(self.conn())
    }

    pub fn add_tx(&self, tx: FranklinTx) -> QueryResult<()> {
        // TODO Check tx and add only txs with valid nonce.
        insert_into(mempool::table)
            .values(&InsertTx {
                hash: tx.hash(),
                primary_account_address: tx.account().data.to_vec(),
                nonce: i64::from(tx.nonce()),
                tx: serde_json::to_value(tx).unwrap(),
            })
            .execute(self.conn())
            .map(drop)
    }

    pub fn get_pending_txs(&self, address: &AccountAddress) -> QueryResult<Vec<FranklinTx>> {
        let pending_txs: Vec<_> = mempool::table
            .left_join(accounts::table.on(accounts::address.eq(address.data.to_vec())))
            .filter(
                accounts::nonce
                    .is_null()
                    .or(accounts::nonce.lt(mempool::nonce)),
            )
            .left_join(
                executed_transactions::table.on(executed_transactions::tx_hash.eq(mempool::hash)),
            )
            .filter(executed_transactions::tx_hash.is_null())
            .order(mempool::nonce.asc())
            .load::<(
                ReadTx,
                Option<StorageAccount>,
                Option<StoredExecutedTransaction>,
            )>(self.conn())?;

        Ok(pending_txs
            .into_iter()
            .map(|(stored_tx, _, _)| serde_json::from_value(stored_tx.tx).unwrap())
            .collect())
    }

    pub fn get_txs(&self, max_size: usize) -> QueryResult<Vec<FranklinTx>> {
        //TODO use "gaps and islands" sql solution for this.
        let stored_txs: Vec<_> = mempool::table
            .left_join(
                executed_transactions::table.on(executed_transactions::tx_hash.eq(mempool::hash)),
            )
            .filter(executed_transactions::tx_hash.is_null())
            .left_join(accounts::table.on(accounts::address.eq(mempool::primary_account_address)))
            .filter(
                accounts::nonce
                    .is_null()
                    .or(accounts::nonce.ge(mempool::nonce)),
            )
            .order(mempool::created_at.asc())
            .limit(max_size as i64)
            .load::<(
                ReadTx,
                Option<StoredExecutedTransaction>,
                Option<StorageAccount>,
            )>(self.conn())?;

        Ok(stored_txs
            .into_iter()
            .map(|stored_tx| serde_json::from_value(stored_tx.0.tx).unwrap())
            .collect())
    }
}
