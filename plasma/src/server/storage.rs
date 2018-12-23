use crate::models::*;
use crate::schema::*;

use diesel::prelude::*;
use diesel::pg::PgConnection;
use dotenv::dotenv;
use std::env;
use serde_json::{to_value, value::Value};
use super::committer;

pub struct StorageConnection {
    conn: PgConnection
}

#[derive(Insertable, Queryable)]
#[table_name="accounts"]
struct Account {
    pub id:     i32,
    pub data:   Value,
}

#[derive(Insertable, Queryable)]
#[table_name="account_updates"]
struct AccountUpdate {
    pub account_id:     i32,
    pub data:           Value,
    pub block_number:   i32,
}

#[derive(Insertable, Queryable)]
#[table_name="operations"]
struct Operation {
    pub id:     Option<i32>,
    pub data:   Value,
    pub addr:   Option<String>,
    pub nonce:  Option<i32>,
}

// TODO: what can go wrong and how to hanlde db save errors?

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

    pub fn commit_op(&self, op: &committer::Operation) -> QueryResult<(String, u32)> {
        diesel::insert_into(operations::table)
            .values(&Operation{
                id:     None,
                data:   to_value(op).unwrap(),
                addr:   None, // will be generated in the database
                nonce:  None, // will be generated in the database
            })
            // TODO: fetch and map to return addr and nonce
            .execute(&self.conn);
            unimplemented!();
    }

    pub fn commit_state_update(&self, block_number: u32, accounts_updated: AccountMap) -> QueryResult<()> {
        for (&account_id, a) in accounts_updated.iter() {
            diesel::insert_into(account_updates::table)
                .values(&AccountUpdate{
                    account_id:     account_id as i32,
                    block_number:   block_number as i32,
                    data:           to_value(a).unwrap(),
                })
                .execute(&self.conn)
                .expect("database must be functional");
        }
        Ok(())
    }

    pub fn apply_state_update(&self, block_number: u32) -> QueryResult<()> {
        // TODO: UPDATE accounts a FROM account_updates u 
        // SET a.data = u.data, a.updated_at = now()
        // WHERE a.id = u.id AND u.block_number = :block_number
        Ok(())
    }

    // TODO: return stream instead
    pub fn load_verified_state() -> AccountMap {
        // TODO: complex select from accounts and account_updates
        AccountMap::default()
    }

    pub fn load_pendings_ops(last_committed_block: u32, last_verified_block: u32) -> Vec<Operation> {
        // TODO: conditional select
        vec![]
    }

}

#[test]
fn storage_test() {
    // let conn = establish_connection();

    // use serde_json::{self, json};

    // use crate::models::Account;

    // use ff::{Field, PrimeField};
    // use pairing::bn256::{Bn256, Fr};

    // let a = Account {
    //     balance: Fr::one(),
    //     nonce: Fr::one(),
    //     pub_x: Fr::one(),
    //     pub_y: Fr::one(),
    // };

    // println!("a = {:#?}", &a);


    // #[derive(Serialize, Deserialize)]
    // pub struct TxUnpacked{
    //     pub from:               u32,
    //     pub to:                 u32,
    //     pub amount:             u32,
    //     pub fee:                u32,
    //     pub nonce:              u32,
    //     pub good_until_block:   u32,

    //     pub sig_r:              String, // r.x
    //     pub sig_s:              String,
    // }

    // let tx = TxUnpacked{
    //     from:            0,
    //     to:              0,
    //     amount:          0,
    //     fee:             0,
    //     nonce:           0,
    //     good_until_block:0,

    //     sig_r:           "0".to_string(),
    //     sig_s:           "0".to_string(),
    // };

    // use crate::models::tx::{self, TxSignature};

    // use sapling_crypto::alt_babyjubjub::{JubjubEngine};

    // #[derive(Serialize, Deserialize)]
    // pub struct Point<E: JubjubEngine, Subgroup> {
    //     x: E::Fr,
    //     y: E::Fr,
    //     t: E::Fr,
    //     z: E::Fr,

    //     #[serde(skip)]
    //     #[serde(bound = "")]
    //     _marker: std::marker::PhantomData<Subgroup>
    // }

    // #[derive(Serialize, Deserialize)]
    // pub struct Tx<E: JubjubEngine> {
    //     pub from:               E::Fr,
    //     pub to:                 E::Fr,
    //     pub amount:             E::Fr, // packed, TODO: document it here
    //     pub fee:                E::Fr, // packed
    //     pub nonce:              E::Fr,
    //     pub good_until_block:   E::Fr,
    //     //pub signature:          TransactionSignature<E>,

    //     #[serde(bound = "")]
    //     pub point:              Point<E, sapling_crypto::jubjub::Unknown>,
    // }

    // let tx2 = tx::Tx::<Bn256> {
    //     from:               Fr::zero(),
    //     to:                 Fr::zero(),
    //     amount:             Fr::zero(), // packed, TODO: document it here
    //     fee:                Fr::zero(), // packed
    //     nonce:              Fr::zero(),
    //     good_until_block:   Fr::zero(),
    //     signature:          TransactionSignature::empty(),

    //     // point:              Point{
    //     //     x: Fr::zero(),
    //     //     y: Fr::zero(),
    //     //     t: Fr::zero(),
    //     //     z: Fr::zero(),
    //     //     _marker: std::marker::PhantomData
    //     // },

    //     //_marker: std::marker::PhantomData,
    // };

    // let v = serde_json::to_value(tx2).unwrap();

    // println!("{}", v.to_string());

    // // use diesel::prelude::*;
    // // use crate::schema::*;
    // // use serde_json::value::Value;

    // // #[derive(Insertable)]
    // // #[table_name="blocks"]
    // // pub struct NewBlock {
    // //     pub block_number:   Option<i32>,
    // //     pub block_data:     Value,
    // // }

    // // let b = NewBlock {
    // //     block_number:   None,
    // //     block_data:     v,
    // // };

    // // let rows_inserted = diesel::insert_into(blocks::table)
    // //     .values(&b)
    // //     .execute(&conn)
    // //     .expect("Error saving account");
    // // println!("{:?}", rows_inserted);

    // // #[derive(Queryable, Debug)]
    // // pub struct Block {
    // //     pub block_number:   i32,
    // //     pub block_data:     Value,
    // // }

    // // {
    // //     use crate::schema::blocks::dsl::*;

    // //     let results = blocks
    // //         //.limit(5)
    // //         .load::<Block>(&conn)
    // //         .expect("Error loading posts");

    // //     println!("{:#?}", results);

    // //     let a: Account = serde_json::from_value(results[results.len()-1].block_data.clone()).unwrap();
    // //     println!("a = {:#?}", &a);
    // // }
}