use plasma::models::*;
use crate::schema::*;
use super::models::{EthOperation, StoredOperation};

use diesel::prelude::*;
use diesel::pg::PgConnection;
use diesel::sql_types::Integer;
use dotenv::dotenv;
use std::env;
use serde_json::{to_value, value::Value};

pub struct StorageConnection {
    conn: PgConnection
}

#[derive(Insertable, QueryableByName)]
#[table_name="accounts"]
struct Account {
    pub id:         i32,
    pub last_block: i32,
    pub data:       Value,
}

#[derive(Insertable, Queryable)]
#[table_name="account_updates"]
struct AccountUpdate {
    pub account_id:     i32,
    pub data:           Value,
    pub block_number:   i32,
}

#[derive(Insertable)]
#[table_name="operations"]
struct NewOperation {
    pub addr:   String,
    pub data:   Value,
}

impl StorageConnection {

    /// creates a single db connection; it's safe to create multiple instances of StorageConnection
   pub fn new() -> Self {
        Self{
            conn: Self::establish_connection()
        }
    }

    fn establish_connection() -> PgConnection {
        dotenv().ok();
        let database_url = env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set");
        PgConnection::establish(&database_url)
            .expect(&format!("Error connecting to {}", database_url))
    }

    pub fn commit_op(&self, op: &EthOperation) -> QueryResult<StoredOperation> {
        self.conn.transaction(|| {
            match &op {
                EthOperation::Commit{block_number, new_root: _, block_data: _, accounts_updated} => 
                    self.commit_state_update(*block_number, accounts_updated)?,
                EthOperation::Verify{block_number, proof: _, block_data: _, accounts_updated: _} => 
                    self.apply_state_update(*block_number)?,
                _ => unimplemented!(),
            };
            diesel::insert_into(operations::table)
                .values(&NewOperation{ addr: "0x0".to_string(), data: serde_json::to_value(&op).unwrap() })
                .get_result(&self.conn)
        })
    }

    fn commit_state_update(&self, block_number: u32, accounts_updated: &AccountMap) -> QueryResult<()> {
        for (&account_id, a) in accounts_updated.iter() {
            diesel::insert_into(account_updates::table)
                .values(&AccountUpdate{
                    account_id:     account_id as i32,
                    block_number:   block_number as i32,
                    data:           to_value(a).unwrap(),
                })
                .execute(&self.conn)
                .expect("must insert into the account updates table");
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
            SET data = EXCLUDED.data", block_number);
        diesel::sql_query(update.as_str())
            .execute(&self.conn)
            .map(|_|())
    }

    pub fn load_committed_state(&self) -> QueryResult<(u32, AccountMap)> {

        // select basis from accounts and join newer state from account_updates for all updates after the last committed block
        const SELECT: &str = "
        SELECT 
            COALESCE(id, u.account_id) AS id,
            COALESCE(u.block_number, a.last_block) AS last_block,   
            COALESCE(u.data, a.data) AS data   
        FROM accounts a
        FULL JOIN account_updates u 
        ON a.id = u.account_id 
        AND u.block_number > (SELECT COALESCE(max(last_block), 0) FROM accounts)";

        diesel::sql_query(SELECT)
            .load(&self.conn)
            .map(|accounts: Vec<Account>| {
                let mut result = AccountMap::default();
                let last_block = accounts.iter().map(|a| a.last_block).max().unwrap() as u32;
                result.extend(accounts.into_iter().map(|a| (
                        a.id as u32, 
                        serde_json::from_value(a.data).unwrap()
                    )));
                (last_block, result)
            })
    }

    pub fn reset_op_config(&self, addr: &str, nonce: u32) -> QueryResult<()> {
        diesel::sql_query(format!("DELETE FROM operations WHERE nonce >= {}", nonce as i32).as_str()).execute(&self.conn)?;
        diesel::sql_query(format!("UPDATE op_config SET addr = '{}', next_nonce = {}", addr, nonce as i32).as_str())
            .execute(&self.conn)
            .map(|_|())
    }

    pub fn load_pendings_ops(&self, current_nonce: u32) -> QueryResult<Vec<StoredOperation>> {
        use crate::schema::operations::dsl::*;
        operations
            .filter(nonce.ge(current_nonce as i32)) // WHERE nonce >= current_nonce
            .load(&self.conn)
    }

}

#[cfg(test)]
mod test {

use diesel::prelude::*;
use plasma::models::{self, AccountMap};
use diesel::Connection;
use bigdecimal::BigDecimal;
use diesel::RunQueryDsl;

fn load_verified_state(conn: &super::StorageConnection) -> QueryResult<AccountMap> {
    diesel::sql_query("SELECT * FROM accounts")
        .load(&conn.conn)
        .map(|accounts: Vec<super::Account>| {
            let mut result = AccountMap::default();
            result.extend(accounts.into_iter().map(|a| (
                    a.id as u32, 
                    serde_json::from_value(a.data).unwrap()
                )));
            result
        })
}

#[test]
fn test_store_state() {
    
    let conn = super::StorageConnection::new();
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

    let state = load_verified_state(&conn).unwrap();
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
    let state = load_verified_state(&conn).unwrap();
    assert_eq!(
        state.into_iter().collect::<Vec<(u32, models::Account)>>(), 
        accounts.clone().into_iter().collect::<Vec<(u32, models::Account)>>());

    // commit second state update
    println!("second");
    let mut accounts2 = fnv::FnvHashMap::default();
    accounts2.insert(2, acc(2));
    accounts2.insert(4, acc(4));
    conn.commit_state_update(2, &accounts2).unwrap();

    assert_eq!(load_verified_state(&conn).unwrap().len(), 3);
    assert_eq!(conn.load_committed_state().unwrap().1.len(), 4);

}

use crate::models::{EthOperation, EthBlockData};
use web3::types::{U256, H256};

#[test]
fn test_store_ops() {

    let conn = super::StorageConnection::new();
    conn.conn.begin_test_transaction().unwrap(); // this will revert db after test

    conn.reset_op_config("0x0", 0).unwrap();

    let commit = conn.commit_op(&EthOperation::Commit{
        block_number:       1, 
        new_root:           H256::zero(), 
        block_data:         EthBlockData::Deposit{batch_number: 0}, 
        accounts_updated:   fnv::FnvHashMap::default()
    }).unwrap();

    let verify = conn.commit_op(&EthOperation::Verify{
        block_number:       1, 
        proof:              [U256::zero(); 8], 
        block_data:         EthBlockData::Deposit{batch_number: 0}, 
        accounts_updated:   fnv::FnvHashMap::default()
    }).unwrap();

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

}