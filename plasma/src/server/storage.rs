use crate::models::*;
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

    use crate::models::Account;

    use ff::{Field, PrimeField};
    use pairing::bn256::{Bn256, Fr};

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