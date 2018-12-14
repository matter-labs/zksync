use crate::models::plasma_sql::*;
use crate::schema::*;

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

    use serde_json::json;

    let b = Block {
        block_number:   2,
        block_data:     json!(1),
    };

    let rows_inserted = diesel::insert_into(blocks::table)
        .values(&b)
        .execute(&conn)
        .expect("Error saving account");
    println!("{:?}", rows_inserted);
}