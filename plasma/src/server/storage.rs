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

    use serde_json::{self, json};

    use crate::models::plasma_models::Account;

    use ff::{Field, PrimeField};
    use pairing::bn256::Fr;

    let a = Account {
        balance: Fr::one(),
        nonce: Fr::one(),
        pub_x: Fr::one(),
        pub_y: Fr::one(),
    };

    #[derive(Serialize, Deserialize)]
    pub struct TxUnpacked{
        pub from:               u32,
        pub to:                 u32,
        pub amount:             u32,
        pub fee:                u32,
        pub nonce:              u32,
        pub good_until_block:   u32,

        pub sig_r:              String, // r.x
        pub sig_s:              String,
    }

    let tx = TxUnpacked{
        from:            0,
        to:              0,
        amount:          0,
        fee:             0,
        nonce:           0,
        good_until_block:0,

        sig_r:           "0".to_string(),
        sig_s:           "0".to_string(),
    };

    let v = serde_json::to_value(tx).unwrap();

    let b = Block {
        block_number:   3,
        block_data:     v,
    };

    let rows_inserted = diesel::insert_into(blocks::table)
        .values(&b)
        .execute(&conn)
        .expect("Error saving account");
    println!("{:?}", rows_inserted);
}