// External imports
// Workspace imports
// Local imports
use crate::schema::*;

#[derive(Debug, Queryable, QueryableByName)]
#[table_name = "mempool_txs"]
pub struct MempoolTx {
    pub id: i64,
    pub tx_hash: String,
    pub tx: serde_json::Value,
}

#[derive(Debug, Insertable)]
#[table_name = "mempool_txs"]
pub struct NewMempoolTx {
    pub tx_hash: String,
    pub tx: serde_json::Value,
}
