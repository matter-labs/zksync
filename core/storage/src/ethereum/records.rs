// External imports
// Workspace imports
// Local imports
use crate::schema::*;
use crate::utils::StoredBigUint;

#[derive(Debug, Clone, Queryable, QueryableByName, PartialEq)]
#[table_name = "eth_operations"]
pub struct StorageETHOperation {
    pub id: i64,
    pub nonce: i64,
    pub confirmed: bool,
    pub raw_tx: Vec<u8>,
    pub op_type: String,
    pub final_hash: Option<Vec<u8>>,
    pub last_deadline_block: i64,
    pub last_used_gas_price: StoredBigUint,
}

#[derive(Debug, Clone, Queryable, QueryableByName, PartialEq)]
#[table_name = "eth_tx_hashes"]
pub struct ETHTxHash {
    pub id: i64,
    pub eth_op_id: i64,
    pub tx_hash: Vec<u8>,
}

#[derive(Debug, Clone, Insertable, PartialEq)]
#[table_name = "eth_tx_hashes"]
pub struct NewETHTxHash {
    pub eth_op_id: i64,
    pub tx_hash: Vec<u8>,
}

#[derive(Debug, Insertable, PartialEq)]
#[table_name = "eth_operations"]
pub struct NewETHOperation {
    pub nonce: i64,
    pub raw_tx: Vec<u8>,
    pub op_type: String,
    pub last_deadline_block: i64,
    pub last_used_gas_price: StoredBigUint,
}

#[derive(Debug, Insertable, PartialEq)]
#[table_name = "eth_ops_binding"]
pub struct NewETHBinding {
    pub op_id: i64,
    pub eth_op_id: i64,
}

#[derive(Debug, Queryable, QueryableByName, PartialEq)]
#[table_name = "eth_ops_binding"]
pub struct ETHBinding {
    pub id: i64,
    pub op_id: i64,
    pub eth_op_id: i64,
}

#[derive(Debug, Queryable, QueryableByName, PartialEq)]
#[table_name = "eth_nonce"]
pub struct ETHNonce {
    pub id: bool,
    pub nonce: i64,
}

#[derive(Debug, Queryable, QueryableByName, PartialEq)]
#[table_name = "eth_stats"]
pub struct ETHStats {
    pub id: bool,
    pub commit_ops: i64,
    pub verify_ops: i64,
    pub withdraw_ops: i64,
}
