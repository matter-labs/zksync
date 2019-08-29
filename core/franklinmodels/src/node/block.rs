use super::operations::FranklinOp;
use super::tx::FranklinTx;
use super::{AccountId, BlockNumber, Fr};
use crate::params::BLOCK_SIZE_CHUNKS;

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
        let mut executed_tx_pub_data = self
            .block_transactions
            .iter()
            .filter_map(|tx| tx.op.clone().map(|op| op.public_data()))
            .fold(Vec::new(), |mut acc, pub_data| {
                acc.extend(pub_data.into_iter());
                acc
            });

        // Pad block with noops.
        executed_tx_pub_data.resize(BLOCK_SIZE_CHUNKS * 8, 0x00);

        executed_tx_pub_data
    }
}
