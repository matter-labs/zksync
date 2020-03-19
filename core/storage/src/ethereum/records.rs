// External imports
use bigdecimal::BigDecimal;
// Workspace imports
// Local imports
use crate::schema::*;

#[derive(Debug, Clone, Queryable, QueryableByName, PartialEq)]
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

#[derive(Debug, Insertable, PartialEq)]
#[table_name = "eth_operations"]
pub struct NewETHOperation {
    pub op_id: i64,
    pub nonce: i64,
    pub deadline_block: i64,
    pub gas_price: BigDecimal,
    pub tx_hash: Vec<u8>,
    pub raw_tx: Vec<u8>,
}

#[derive(Debug, Insertable, PartialEq)]
#[table_name = "eth_nonce"]
pub struct NewETHNonce {
    pub nonce: i64,
}

#[derive(Debug, Queryable, QueryableByName, PartialEq)]
#[table_name = "eth_nonce"]
pub struct ETHNonce {
    pub id: bool,
    pub nonce: i64,
}
