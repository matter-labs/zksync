// External imports
use chrono::prelude::*;
use diesel::sql_types::{BigInt, Binary, Nullable, Timestamp};
use serde_derive::{Deserialize, Serialize};
// Workspace imports
// Local imports
use crate::schema::*;

#[derive(Debug, Insertable, Queryable)]
#[table_name = "blocks"]
pub struct StorageBlock {
    pub number: i64,
    pub root_hash: Vec<u8>,
    pub fee_account_id: i64,
    pub unprocessed_prior_op_before: i64,
    pub unprocessed_prior_op_after: i64,
    pub block_size: i64,
}

#[derive(Debug, Serialize, Deserialize, QueryableByName, PartialEq)]
pub struct BlockDetails {
    #[sql_type = "BigInt"]
    pub block_number: i64,

    #[sql_type = "Binary"]
    pub new_state_root: Vec<u8>,

    #[sql_type = "BigInt"]
    pub block_size: i64,

    #[sql_type = "Nullable<Binary>"]
    pub commit_tx_hash: Option<Vec<u8>>,

    #[sql_type = "Nullable<Binary>"]
    pub verify_tx_hash: Option<Vec<u8>>,

    #[sql_type = "Timestamp"]
    pub committed_at: NaiveDateTime,

    #[sql_type = "Nullable<Timestamp>"]
    pub verified_at: Option<NaiveDateTime>,
}
