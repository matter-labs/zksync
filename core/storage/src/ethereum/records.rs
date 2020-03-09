// External imports
use bigdecimal::BigDecimal;
use serde_derive::{Deserialize, Serialize};
// Workspace imports
// Local imports
use crate::schema::*;

#[derive(Debug, Clone, Queryable, QueryableByName)]
#[table_name = "eth_operations"]
pub struct StorageETHOperation {
    pub id: i64,
    pub op_id: i64,
    pub nonce: i64,
    pub deadline_block: i64,
    pub gas_price: BigDecimal,
    pub tx_hash: Vec<u8>,
    pub confirmed: bool,
    pub raw_tx: Vec<u8>,
}

#[derive(Debug, Insertable)]
#[table_name = "eth_operations"]
pub struct NewETHOperation {
    pub op_id: i64,
    pub nonce: i64,
    pub deadline_block: i64,
    pub gas_price: BigDecimal,
    pub tx_hash: Vec<u8>,
    pub raw_tx: Vec<u8>,
}

#[derive(Insertable)]
#[table_name = "data_restore_last_watched_eth_block"]
pub struct NewLastWatchedEthBlockNumber {
    pub block_number: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Queryable, QueryableByName)]
#[table_name = "data_restore_last_watched_eth_block"]
pub struct StoredLastWatchedEthBlockNumber {
    pub id: i32,
    pub block_number: String,
}
