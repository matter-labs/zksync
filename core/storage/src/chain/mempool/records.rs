// External imports
// Workspace imports
// Local imports
use crate::schema::*;

#[derive(Debug, Queryable, QueryableByName, Insertable)]
#[table_name = "mempool_txs"]
pub struct MempoolTx {
    pub tx_hash: String,
    pub tx: serde_json::Value,
}
