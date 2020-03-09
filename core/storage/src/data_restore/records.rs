// External imports
// Workspace imports
use models::node::{AccountId, BlockNumber, FranklinOp};

#[derive(Debug, Clone, Queryable)]
pub struct StoredRollupOpsBlock {
    pub block_num: BlockNumber,
    pub ops: Vec<FranklinOp>,
    pub fee_account: AccountId,
}
