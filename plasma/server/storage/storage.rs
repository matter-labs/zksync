extern crate server_models;
extern crate plasma;
extern crate fnv;

#[macro_use]
extern crate diesel;
extern crate bigdecimal;
extern crate serde_json;

use plasma::models::*;
use server_models::{Operation, Action};

mod schema;
use schema::*;

use diesel::prelude::*;
use diesel::pg::PgConnection;
use diesel::sql_types::Integer;
use diesel::result::Error;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};

use serde_json::{to_value, value::Value};
use std::env;
use std::iter::FromIterator;

#[derive(Clone)]
pub struct ConnectionPool {
    pub pool: Pool<ConnectionManager<PgConnection>>, 
}

impl ConnectionPool {
    pub fn new() -> Self {
        let database_url = env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set");

        let manager = ConnectionManager::<PgConnection>::new(database_url);
        let pool = Pool::builder().build(manager).expect("Failed to create connection pool");
        
        Self {
            pool
        }
    }
}

#[derive(Insertable, QueryableByName, Queryable)]
#[table_name="accounts"]
struct Account {
    pub id:         i32,
    pub last_block: i32,
    pub data:       Value,
}

#[derive(Insertable, Queryable, QueryableByName)]
#[table_name="account_updates"]
struct AccountUpdate {
    pub account_id:     i32,
    pub data:           Value,
    pub block_number:   i32,
}

#[derive(Insertable)]
#[table_name="operations"]
struct NewOperation {
    pub data:           Value,
    pub block_number:   i32,
    pub action_type:    String,
}

#[derive(Debug, Queryable, QueryableByName)]
#[table_name="operations"]
pub struct StoredOperation {
    pub id:             i32,
    pub data:           serde_json::Value,
    pub addr:           String,
    pub nonce:          i32,
    pub block_number:   i32,
    pub action_type:    String,
    pub created_at:     std::time::SystemTime,
}

impl StoredOperation {
    pub fn get_meta(&self) -> TxMeta {
        TxMeta{
            addr:   self.addr.clone(), 
            nonce:  self.nonce as u32,
        }
    }

    pub fn into_op(self, conn: &StorageProcessor) -> QueryResult<Operation> {
        let meta = self.get_meta();
        let mut op: Operation = serde_json::from_value(self.data).unwrap();
        op.tx_meta = Some(meta);

        if op.accounts_updated.is_none() {
            let (_, updates) = conn.load_state_at_block(op.block.block_number)?;
            op.accounts_updated = Some(updates);
        };

        Ok(op)
    }
}

#[derive(Debug, QueryableByName)]
pub struct IntegerNumber {
    #[sql_type="Integer"]
    pub integer_value: i32,
}


pub struct StorageProcessor {
    conn:  PooledConnection<ConnectionManager<PgConnection>>,
}

impl StorageProcessor {

    pub fn from_connection(conn: PooledConnection<ConnectionManager<PgConnection>>) -> Self {
        Self {
            conn: conn
        }
    }

    pub fn commit_and_augument_op(&self, op: &Operation) -> QueryResult<Operation> {

        self.conn.transaction(|| {
            match &op.action {
                Action::Commit => 
                    self.commit_state_update(op.block.block_number, op.accounts_updated.as_ref().unwrap())?,
                Action::Verify{proof: _} => 
                    self.apply_state_update(op.block.block_number)?,
            };
            let stored: StoredOperation = diesel::insert_into(operations::table)
                .values(&NewOperation{ 
                    block_number:   op.block.block_number as i32,
                    action_type:    op.action.to_string(),
                    data:           serde_json::to_value(&op).unwrap(), 
                })
                .get_result(&self.conn)?;
            stored.into_op(self)
        })
    }

    fn commit_state_update(&self, block_number: u32, accounts_updated: &AccountMap) -> QueryResult<()> {
        for (&account_id, a) in accounts_updated.iter() {
            println!("Committing state update for account {} in block {}", account_id, block_number);
            let inserted = diesel::insert_into(account_updates::table)
                .values(&AccountUpdate{
                    account_id:     account_id as i32,
                    block_number:   block_number as i32,
                    data:           to_value(a).unwrap(),
                })
                .execute(&self.conn)?;
            if 0 == inserted {
                eprintln!("Error: could not commit all state updates!");
                return Err(Error::RollbackTransaction)
            }
        }
        Ok(())
    }

    fn apply_state_update(&self, block_number: u32) -> QueryResult<()> {
        let update = format!("
            INSERT INTO accounts (id, last_block, data)
            SELECT 
                account_id AS id, 
                block_number as last_block, 
                data FROM account_updates
            WHERE account_updates.block_number = {}
            ON CONFLICT (id) 
            DO UPDATE 
            SET data = EXCLUDED.data, last_block = EXCLUDED.last_block", block_number);
        diesel::sql_query(update.as_str())
            .execute(&self.conn)
            .map(|_|())
    }


    pub fn load_committed_state(&self) -> QueryResult<(u32, AccountMap)> {
        const SELECT: &str = "
        WITH upd AS (
            WITH s AS (
                SELECT account_id as id, max(block_number) as last_block 
                FROM account_updates u 
                WHERE u.block_number > (SELECT COALESCE(max(last_block), 0) FROM accounts) 
                GROUP BY account_id
            ) 
            SELECT u.account_id AS id, u.block_number AS last_block, u.data FROM s, account_updates u WHERE s.id = u.account_id AND u.block_number = s.last_block
        )
        SELECT COALESCE(u.id, a.id) AS id, COALESCE(u.last_block, a.last_block) AS last_block, COALESCE (u.data, a.data) AS data
        FROM upd u
        FULL JOIN accounts a ON a.id = u.id
        ORDER BY id";

        self.load_state(SELECT)
    }

    pub fn load_verified_state(&self) -> QueryResult<(u32, AccountMap)> {
        self.load_state("SELECT * FROM accounts a")
    }

    /// loads the state of accounts updated between two blocks: (start, end]
    pub fn load_state_diff(&self, start_block: u32, end_block: u32) -> QueryResult<(u32, AccountMap)> {
        let select = format!("
        WITH upd AS (
            WITH s AS (
                SELECT account_id as id, max(block_number) as last_block 
                FROM account_updates u 
                WHERE u.block_number >= {} AND u.block_number < {}
                GROUP BY account_id
            ) 
            SELECT u.account_id AS id, u.block_number AS last_block, u.data FROM s, account_updates u WHERE s.id = u.account_id AND u.block_number = s.last_block
        )
        SELECT u.id, u.last_block, u.data
        FROM upd u
        ORDER BY id", start_block, end_block);

        self.load_state(select.as_str())
    }

    /// loads the state of accounts updated in a specific block
    pub fn load_state_at_block(&self, block_number: u32) -> QueryResult<(u32, AccountMap)> {
        self.load_state_diff(block_number-1, block_number)
    }

    fn load_state(&self, query: &str) -> QueryResult<(u32, AccountMap)> {
        let r = diesel::sql_query(query)
            .load(&self.conn)
            .map(|accounts: Vec<Account>| {
                let last_block = accounts.iter().map(|a| a.last_block as u32).max().unwrap_or(0);
                let result = AccountMap::from_iter(accounts.into_iter().map(|a| (
                        a.id as u32, 
                        serde_json::from_value(a.data).unwrap()
                    )));
                (last_block, result)
            });
        r
    }

    pub fn update_op_config(&self, addr: &str, nonce: u32) -> QueryResult<()> {
        diesel::sql_query(format!("
            UPDATE op_config 
            SET addr = '{addr}', next_nonce = s.next_nonce
            FROM (
                SELECT max(max_nonce) AS next_nonce
                FROM (
                    SELECT max(nonce) AS max_nonce 
                    FROM operations WHERE addr = '{addr}' 
                    UNION SELECT {nonce} AS max_nonce
                ) t
            ) s", addr = addr, nonce = nonce as i32).as_str())
            .execute(&self.conn)
            .map(|_|())
    }

    pub fn load_pendings_ops(&self, current_nonce: u32) -> QueryResult<Vec<Operation>> {
        use crate::schema::operations::dsl::*;
        self.conn.transaction(|| {
            let ops: Vec<StoredOperation> = operations
                .filter(nonce.ge(current_nonce as i32)) // WHERE nonce >= current_nonce
                .load(&self.conn)?;
            ops.into_iter().map(|o| o.into_op(self)).collect()
        })
    }

    pub fn load_pendings_proof_reqs(&self) -> QueryResult<Vec<Operation>> {
        self.conn.transaction(|| {
            let ops: Vec<StoredOperation> = diesel::sql_query("
                SELECT * FROM operations
                WHERE action_type = 'Commit'
                AND block_number > (
                    SELECT COALESCE(max(block_number), 0)  
                    FROM operations 
                    WHERE action_type = 'Verify'
                )
            ").load(&self.conn)?;
            ops.into_iter().map(|o| o.into_op(self)).collect()
        })
    }

    fn load_number(&self, query: &str) -> QueryResult<i32> {
        diesel::sql_query(query)
            .get_result::<IntegerNumber>(&self.conn)
            .map(|r| r.integer_value)
    }

    pub fn load_last_committed_deposit_batch(&self) -> QueryResult<i32> {
        self.load_number("
            SELECT COALESCE(max((data->'block'->'block_data'->>'batch_number')::int), -1) as integer_value FROM operations 
            WHERE data->'action'->>'type' = 'Commit' 
            AND data->'block_data'->>'type' = 'Deposit'
        ")
    }

    pub fn load_last_committed_exit_batch(&self) -> QueryResult<i32> {
        self.load_number("
            SELECT COALESCE(max((data->'block'->'block_data'->>'batch_number')::int), -1) as integer_value FROM operations 
            WHERE data->'action'->>'type' = 'Commit' 
            AND data->'block_data'->>'type' = 'Exit'
        ")
    }

    pub fn last_committed_state_for_account(&self, account_id: u32) -> QueryResult<Option<plasma::models::Account>> {
        let query = format!("
            SELECT account_id AS id, block_number AS last_block, data
            FROM account_updates WHERE account_id = {}
            ORDER BY block_number DESC LIMIT 1
        ", account_id);
        let r = diesel::sql_query(query)
            .get_result(&self.conn)
            .optional()?;
        Ok( r.map(|acc: Account| serde_json::from_value(acc.data).unwrap()) )
    }

    pub fn last_verified_state_for_account(&self, account_id: u32) -> QueryResult<Option<plasma::models::Account>> {
        use crate::schema::accounts::dsl::*;
        let mut r = accounts
            .filter(id.eq(account_id as i32))
            .load(&self.conn)?;
        Ok( r.pop().map(|acc: Account| serde_json::from_value(acc.data).unwrap()) )
    }

    pub fn get_last_committed_block(&self) -> QueryResult<i32> {
        self.load_number("SELECT COALESCE(max(block_number), 0) AS integer_value FROM account_updates")
    }

    pub fn get_last_verified_block(&self) -> QueryResult<i32> {
        self.load_number("SELECT COALESCE(max(last_block), 0) AS integer_value FROM accounts")
    }

}

#[cfg(test)]
mod test {

    use diesel::prelude::*;
    use plasma::models::{self, AccountMap};
    use diesel::Connection;
    use bigdecimal::BigDecimal;
    use diesel::RunQueryDsl;

    #[test]
    fn test_store_state() {
        
        let pool = super::ConnectionPool::new();

        let conn = super::StorageProcessor::from_connection(pool.pool.get().unwrap());
        conn.conn.begin_test_transaction().unwrap(); // this will revert db after test

        // uncomment below for debugging to generate initial state
        diesel::sql_query("delete from accounts")
        .execute(&conn.conn)
        .expect("must work");
        diesel::sql_query("delete from account_updates")
        .execute(&conn.conn)
        .expect("must work");

        let mut accounts = fnv::FnvHashMap::default();
        let acc = |balance| { 
            let mut a = models::Account::default(); 
            a.balance = BigDecimal::from(balance);
            a
        };

        // commit initial state update
        accounts.insert(1, acc(1));
        accounts.insert(2, acc(2));
        accounts.insert(3, acc(3));
        conn.commit_state_update(1, &accounts).unwrap();

        let (_, state) = conn.load_verified_state().unwrap();
        assert_eq!(state.len(), 0);
        
        // committed state must be computed from updates
        let (last_block, state) = conn.load_committed_state().unwrap();
        assert_eq!(last_block, 1);
        assert_eq!(
            state.into_iter().collect::<Vec<(u32, models::Account)>>(), 
            accounts.clone().into_iter().collect::<Vec<(u32, models::Account)>>());

        // now apply commitment
        conn.apply_state_update(1).expect("update must work");
        
        // verified state must be equal the commitment
        let (_, state) = conn.load_verified_state().unwrap();
        assert_eq!(
            state.into_iter().collect::<Vec<(u32, models::Account)>>(), 
            accounts.clone().into_iter().collect::<Vec<(u32, models::Account)>>());

        let (_, state) = conn.load_state_diff(1, 2).unwrap();
        assert_eq!( state.get(&2).unwrap(), &acc(2) );

        // commit second state update
        let mut accounts2 = fnv::FnvHashMap::default();
        accounts2.insert(2, acc(23));
        accounts2.insert(4, acc(4));
        conn.commit_state_update(2, &accounts2).unwrap();

        assert_eq!(conn.load_verified_state().unwrap().1.len(), 3);
        assert_eq!(conn.load_committed_state().unwrap().1.len(), 4);

        let (_, state) = conn.load_state_diff(1, 2).unwrap();
        assert_eq!( state.get(&2).unwrap(), &acc(2) );
        let (_, state) = conn.load_state_diff(1, 3).unwrap();
        assert_eq!( state.get(&2).unwrap(), &acc(23) );
        let (_, state) = conn.load_state_diff(2, 3).unwrap();
        assert_eq!( state.get(&2).unwrap(), &acc(23) );
    }

    use plasma::models::{Block, DepositBlock, U256, H256};
    use crate::models::{Operation, EthBlockData, Action};

    #[test]
    fn test_store_txs() {
        let pool = super::ConnectionPool::new();

        let conn = super::StorageProcessor::from_connection(pool.pool.get().unwrap());
        conn.conn.begin_test_transaction().unwrap(); // this will revert db after test
        conn.update_op_config("0x0", 0).unwrap();

        let mut accounts = fnv::FnvHashMap::default();
        let acc = |balance| { 
            let mut a = models::Account::default(); 
            a.balance = BigDecimal::from(balance);
            a
        };

        accounts.insert(3, acc(1));
        accounts.insert(5, acc(2));
        accounts.insert(7, acc(3));
        accounts.insert(8, acc(4));
        let commit = conn.commit_op(&Operation{
            action: Action::Commit{
                new_root:   H256::zero(), 
                block:      None,
            },
            block_number:       1, 
            block_data:         EthBlockData::Deposit{batch_number: 0}, 
            accounts_updated:   accounts.clone()
        }).unwrap();

        assert_eq!(conn.last_verified_state_for_account(5).unwrap(), None);
        assert_eq!(conn.last_committed_state_for_account(5).unwrap().unwrap().balance, BigDecimal::from(2));

        let verify = conn.commit_op(&Operation{
            action: Action::Verify{
                proof: [U256::zero(); 8], 
            },
            block_number:       1, 
            block_data:         EthBlockData::Deposit{batch_number: 0}, 
            accounts_updated:   accounts.clone()
        }).unwrap();

        assert_eq!(conn.last_verified_state_for_account(7).unwrap().unwrap().balance, BigDecimal::from(3));
        assert_eq!(conn.last_committed_state_for_account(7).unwrap().unwrap().balance, BigDecimal::from(3));

        let pending = conn.load_pendings_ops(0).unwrap();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].nonce, 0);
        assert_eq!(pending[1].nonce, 1);

        let pending = conn.load_pendings_ops(1).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].nonce, 1);

        let pending = conn.load_pendings_ops(2).unwrap();
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn test_store_proof_reqs() {
        let pool = super::ConnectionPool::new();

        let conn = super::StorageProcessor::from_connection(pool.pool.get().unwrap());

        conn.conn.begin_test_transaction().unwrap(); // this will revert db after test
        conn.update_op_config("0x0", 0).unwrap();

        let commit = conn.commit_op(&Operation{
            action: Action::Commit{
                new_root:   H256::zero(), 
                block:      Some(Block::Deposit(DepositBlock::default(), 1)),
            },
            block_number:       1, 
            block_data:         EthBlockData::Deposit{batch_number: 1}, 
            accounts_updated:   fnv::FnvHashMap::default()
        }).unwrap();

        let pending = conn.load_pendings_proof_reqs().unwrap();
        assert_eq!(pending.len(), 1);

        let verify = conn.commit_op(&Operation{
            action: Action::Verify{
                proof: [U256::zero(); 8], 
            },
            block_number:       1, 
            block_data:         EthBlockData::Deposit{batch_number: 0}, 
            accounts_updated:   fnv::FnvHashMap::default()
        }).unwrap();

        let pending = conn.load_pendings_proof_reqs().unwrap();
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn test_storage_helpers() {
        let pool = super::ConnectionPool::new();

        let conn = super::StorageProcessor::from_connection(pool.pool.get().unwrap());

        assert_eq!(-1, conn.load_last_committed_deposit_batch().unwrap());
        assert_eq!(-1, conn.load_last_committed_exit_batch().unwrap());
        assert_eq!(0, conn.get_last_committed_block().unwrap());
        assert_eq!(0, conn.get_last_verified_block().unwrap());

        assert_eq!(conn.last_committed_state_for_account(9999).unwrap(), None);
        assert_eq!(conn.last_verified_state_for_account(9999).unwrap(), None);
    }

}