use super::operations::FranklinOp;
use super::{AccountId, BlockNumber, Fr};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub block_number: BlockNumber,
    pub new_root_hash: Fr,
    pub fee_account: AccountId,
    pub block_transactions: Vec<FranklinOp>,
}

impl Block {
    pub fn get_eth_public_data(&self) -> Vec<u8> {
        unimplemented!()
    }
}
