use bigdecimal::BigDecimal;
use diesel::backend::Backend;
use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::{Integer, Record, Text};
use std::io;
use std::io::Write;

use crate::schema::*;
use diesel::prelude::*;

use serde_json::value::Value;


pub struct Account {
    pub id:                 i32,            //u32,
    pub last_block_number:  Option<i32>,    //u32,
    
    pub nonce:              i32,            //u32,
    pub balance:            BigDecimal,

    pub pub_x:              Option<BigDecimal>,
    pub pub_y:              Option<BigDecimal>, 
}

    // -- created_at          timestamp,
    // account_id          integer,        -- account of the tx sender
    // dst_id              integer,        -- for updates only: destination = tx.to
    // amount              numeric(80),    -- amount of the tx
    // pub_x               numeric(80),    -- for registrations only: pub key
    // nonce               bigint,
    // valid_until_block   integer,
    // sig_r               numeric(80),
    // sig_s               numeric(80)



pub struct Tx {
    pub account_id: i32,
}


#[derive(Insertable, Queryable)]
#[table_name="blocks"]
pub struct Block {
    pub block_number:   i32,
    pub block_data:     Value,
}


// #[derive(Debug, Copy, Clone, AsExpression, FromSqlRow)]
// #[sql_type = "SmallInt"]
// pub struct RecordType {
//     repr Engine::Fr;
// }

// impl<DB: Backend> ToSql<SmallInt, DB> for RecordType
// where
//     i16: ToSql<SmallInt, DB>,
// {
//     fn to_sql<W>(&self, out: &mut Output<W, DB>) -> serialize::Result
//     where
//         W: io::Write,
//     {
//         let v = 0;
//         v.to_sql(out)
//     }
// }

// impl<DB: Backend> FromSql<SmallInt, DB> for RecordType
// where
//     i16: FromSql<SmallInt, DB>,
// {
//     fn from_sql(bytes: Option<&DB::RawValue>) -> deserialize::Result<Self> {
//         let v = i16::from_sql(bytes)?;
//         Ok(match v {
//             0 => RecordType{repr: Engine::Fr::zero()},
//             _ => return Err("replace me with a real error".into()),
//         })
//     }
// }

// #[derive(Insertable, Queryable, Debug)]
// #[table_name = "records"]
// pub struct Record {
//     pub id: i64,
//     pub record_type: RecordType,
// }
