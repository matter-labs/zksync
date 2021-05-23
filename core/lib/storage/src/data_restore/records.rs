// External imports
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
// Workspace imports
use zksync_types::{AccountId, Address, BlockNumber, TokenId, ZkSyncOp, H256};
// Workspace imports
// Local imports

#[derive(Debug)]
pub struct NewTokenEvent {
    pub address: Address,
    pub id: TokenId,
}

#[derive(Debug)]
pub struct NewRollupOpsBlock<'a> {
    pub block_num: BlockNumber,
    pub ops: &'a [ZkSyncOp],
    pub fee_account: AccountId,
    pub timestamp: Option<u64>,
    pub previous_block_root_hash: H256,
}

#[derive(Debug, Clone, FromRow)]
pub struct StoredRollupOpsBlock {
    pub block_num: i64,
    pub ops: Option<Vec<Value>>,
    pub fee_account: i64,
    pub timestamp: Option<i64>,
    pub previous_block_root_hash: Option<Vec<u8>>,
    pub contract_version: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow, PartialEq)]
pub struct StoredLastWatchedEthBlockNumber {
    pub id: i32,
    pub block_number: String,
}

#[derive(Debug)]
pub struct NewStorageState {
    pub storage_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StoredStorageState {
    pub id: i32,
    pub storage_state: String,
}

#[derive(Debug)]
pub struct NewBlockEvent {
    pub block_type: String, // 'Committed', 'Verified'
    pub transaction_hash: Vec<u8>,
    pub block_num: i64,
    pub contract_version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StoredBlockEvent {
    pub id: i32,
    pub block_type: String, // 'Committed', 'Verified'
    pub transaction_hash: Vec<u8>,
    pub block_num: i64,
    pub contract_version: i32,
}
