//! This module contains the structures that represent the contents
//! of the tables. Each structure is associated with one of the tables
//! used in project and is used to interact with the database.

// External imports
// Workspace imports
use models::node::{AccountId, BlockNumber, FranklinOp};

pub mod records;

pub use self::records::*;

#[derive(Debug, Clone, Queryable)]
pub struct StoredRollupOpsBlock {
    pub block_num: BlockNumber,
    pub ops: Vec<FranklinOp>,
    pub fee_account: AccountId,
}
