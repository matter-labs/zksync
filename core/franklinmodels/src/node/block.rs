use super::operations::FranklinOp;
use super::tx::FranklinTx;
use super::{AccountId, BlockNumber, Fr};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutedTx {
    pub tx: FranklinTx,
    pub success: bool,
    pub op: Option<FranklinOp>,
    pub fail_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub block_number: BlockNumber,
    pub new_root_hash: Fr,
    pub fee_account: AccountId,
    pub block_transactions: Vec<ExecutedTx>,
}

impl Block {
    pub fn get_eth_public_data(&self) -> Vec<u8> {
        // TODO unimplemented
        Vec::new()
    }
}
