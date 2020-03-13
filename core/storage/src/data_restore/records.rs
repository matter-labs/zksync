// External imports
// Workspace imports
use models::node::{AccountId, BlockNumber, FranklinOp};
use serde_derive::{Deserialize, Serialize};
// Workspace imports
// Local imports
use crate::schema::*;

#[derive(Debug, Clone, Queryable)]
pub struct StoredRollupOpsBlock {
    pub block_num: BlockNumber,
    pub ops: Vec<FranklinOp>,
    pub fee_account: AccountId,
}

#[derive(Debug, Insertable, PartialEq)]
#[table_name = "data_restore_last_watched_eth_block"]
pub struct NewLastWatchedEthBlockNumber {
    pub block_number: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Queryable, QueryableByName, PartialEq)]
#[table_name = "data_restore_last_watched_eth_block"]
pub struct StoredLastWatchedEthBlockNumber {
    pub id: i32,
    pub block_number: String,
}
