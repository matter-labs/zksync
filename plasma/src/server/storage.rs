use crate::models::plasma_sql::{Account};
use crate::schema::accounts;

use diesel::prelude::*;
use diesel::pg::PgConnection;
use dotenv::dotenv;
use std::env;

pub fn establish_connection() -> PgConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

struct StateStorage {

}

impl StateStorage {

    /// creates connection pool
    pub fn new() -> Self {
        Self{}
    }

    // /// returns promise
    // pub fn commit_block(block: &Block) {

    // }

    // /// returns promise
    // pub fn update_state(state: &PlasmaState) {

    // }

    // /// returns stream of accounts
    // pub fn load_state() {

    // }

}

#[test]
fn storage_test() {
    let conn = establish_connection();

    // let a = Account{
    //     id:                 0,
    //     balance:            20,
    //     nonce:              0,
    //     last_block_number:  1,
    //     pub_x:              "10".to_owned(),
    //     pub_y:              "10".to_owned(), 
    // };

    // diesel::insert_into(accounts::table)
    //     .values(&a)
    //     .get_result(&conn)
    //     .expect("Error saving account");
}