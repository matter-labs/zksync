use super::tx::FranklinTx;
use super::{BlockNumber, Fr, AccountId};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub block_number: BlockNumber,
    pub new_root_hash: Fr,
    pub operator_account_id: AccountId,
    pub block_transactions: Vec<FranklinTx>,
}

impl Block {
    pub fn get_eth_public_data(&self) -> Vec<u8> {
        unimplemented!()
    }
}
