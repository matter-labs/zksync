// External imports
use serde_json::Value;
// Workspace imports
use models::node::{AccountId, BlockNumber, FranklinOp};
use serde_derive::{Deserialize, Serialize};
use sqlx::FromRow;
// Workspace imports
// Local imports

#[derive(Debug, Clone, FromRow)]
pub struct StoredRollupOpsBlock {
    pub block_num: BlockNumber,
    pub ops: Vec<FranklinOp>,
    pub fee_account: AccountId,
}

// #[derive(Debug, Insertable, PartialEq)]
// #[table_name = "data_restore_last_watched_eth_block"]
// pub struct NewLastWatchedEthBlockNumber {
//     pub block_number: String,
// }

#[derive(Debug, Serialize, Deserialize, Clone, FromRow, PartialEq)]
pub struct StoredLastWatchedEthBlockNumber {
    pub id: i32,
    pub block_number: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct StoredFranklinOp {
    pub id: i32,
    pub block_num: i64,
    pub operation: Value,
    pub fee_account: i64,
}

impl StoredFranklinOp {
    pub fn into_franklin_op(self) -> FranklinOp {
        serde_json::from_value(self.operation).expect("Unparsable FranklinOp in db")
    }
}
#[derive(Debug, Clone)]
pub struct NewFranklinOp {
    pub block_num: i64,
    pub operation: Value,
    pub fee_account: i64,
}

impl NewFranklinOp {
    pub fn prepare_stored_op(
        franklin_op: &FranklinOp,
        block: BlockNumber,
        fee_account: AccountId,
    ) -> Self {
        Self {
            block_num: i64::from(block),
            operation: serde_json::to_value(franklin_op.clone()).unwrap(),
            fee_account: i64::from(fee_account),
        }
    }
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
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StoredBlockEvent {
    pub id: i32,
    pub block_type: String, // 'Committed', 'Verified'
    pub transaction_hash: Vec<u8>,
    pub block_num: i64,
}
