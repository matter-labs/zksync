use super::FranklinOp;
use super::FranklinTx;
use super::PriorityOp;
use super::{AccountId, BlockNumber, Fr};
use crate::params::BLOCK_SIZE_CHUNKS;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutedTx {
    pub tx: FranklinTx,
    pub success: bool,
    pub op: Option<FranklinOp>,
    pub fail_reason: Option<String>,
    pub block_index: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutedPriorityOp {
    pub priority_op: PriorityOp,
    pub op: FranklinOp,
    pub block_index: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ExecutedOperations {
    Tx(ExecutedTx),
    PriorityOp(ExecutedPriorityOp),
}

impl ExecutedOperations {
    fn get_eth_public_data(&self) -> Vec<u8> {
        match self {
            ExecutedOperations::Tx(exec_tx) => {
                if let Some(op) = &exec_tx.op {
                    op.public_data()
                } else {
                    Vec::new()
                }
            }
            ExecutedOperations::PriorityOp(exec_op) => exec_op.op.public_data(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub block_number: BlockNumber,
    pub new_root_hash: Fr,
    pub fee_account: AccountId,
    pub block_transactions: Vec<ExecutedOperations>,
    /// (unprocessed prior op id before block, unprocessed prior op id after block)
    pub processed_priority_ops: (u64, u64),
}

impl Block {
    pub fn get_eth_public_data(&self) -> Vec<u8> {
        let mut executed_tx_pub_data = self
            .block_transactions
            .iter()
            .map(|tx| tx.get_eth_public_data())
            .fold(Vec::new(), |mut acc, pub_data| {
                acc.extend(pub_data.into_iter());
                acc
            });

        // Pad block with noops.
        executed_tx_pub_data.resize(BLOCK_SIZE_CHUNKS * 8, 0x00);

        executed_tx_pub_data
    }

    pub fn number_of_processed_prior_ops(&self) -> u64 {
        self.processed_priority_ops.1 - self.processed_priority_ops.0
    }
}
