// External imports
use chrono::prelude::*;
use diesel::sql_types::{BigInt, Binary, Jsonb, Nullable, Text, Timestamp};
use serde_derive::{Deserialize, Serialize};
use serde_json::value::Value;
// Workspace imports
// Local imports
use crate::schema::*;
use crate::utils::{BytesToHexSerde, OptionBytesToHexSerde, SyncBlockPrefix, ZeroxPrefix};

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

#[derive(Debug, Serialize, Deserialize, QueryableByName, PartialEq, Clone)]
pub struct BlockDetails {
    #[sql_type = "BigInt"]
    pub block_number: i64,

    #[sql_type = "Binary"]
    #[serde(with = "BytesToHexSerde::<SyncBlockPrefix>")]
    pub new_state_root: Vec<u8>,

    #[sql_type = "BigInt"]
    pub block_size: i64,

    #[sql_type = "Nullable<Binary>"]
    #[serde(with = "OptionBytesToHexSerde::<ZeroxPrefix>")]
    pub commit_tx_hash: Option<Vec<u8>>,

    #[sql_type = "Nullable<Binary>"]
    #[serde(with = "OptionBytesToHexSerde::<ZeroxPrefix>")]
    pub verify_tx_hash: Option<Vec<u8>>,

    #[sql_type = "Timestamp"]
    pub committed_at: NaiveDateTime,

    #[sql_type = "Nullable<Timestamp>"]
    pub verified_at: Option<NaiveDateTime>,
}

#[derive(Debug, Serialize, Deserialize, QueryableByName)]
pub struct BlockTransactionItem {
    #[sql_type = "Text"]
    pub tx_hash: String,

    #[sql_type = "BigInt"]
    pub block_number: i64,

    #[sql_type = "Jsonb"]
    pub op: Value,

    #[sql_type = "Timestamp"]
    pub created_at: NaiveDateTime,
}
