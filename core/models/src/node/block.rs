use super::FranklinOp;
use super::FranklinTx;
use super::PriorityOp;
use super::{AccountId, BlockNumber, Fr};
use crate::franklin_crypto::bellman::pairing::ff::{PrimeField, PrimeFieldRepr};
use crate::params::block_size_chunks;
use crate::serialization::*;
use web3::types::H256;

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
#[serde(tag = "type")]
pub enum ExecutedOperations {
    Tx(Box<ExecutedTx>),
    PriorityOp(Box<ExecutedPriorityOp>),
}

impl ExecutedOperations {
    pub fn get_executed_op(&self) -> Option<&FranklinOp> {
        match self {
            ExecutedOperations::Tx(exec_tx) => exec_tx.op.as_ref(),
            ExecutedOperations::PriorityOp(exec_op) => Some(&exec_op.op),
        }
    }

    pub fn get_eth_public_data(&self) -> Vec<u8> {
        self.get_executed_op()
            .map(|op| op.public_data())
            .unwrap_or_default()
    }

    pub fn get_eth_witness_bytes(&self) -> Option<Vec<u8>> {
        self.get_executed_op().map(|op| op.eth_witness())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub block_number: BlockNumber,
    #[serde(with = "FrSerde")]
    pub new_root_hash: Fr,
    pub fee_account: AccountId,
    pub block_transactions: Vec<ExecutedOperations>,
    /// (unprocessed prior op id before block, unprocessed prior op id after block)
    pub processed_priority_ops: (u64, u64),
}

impl Block {
    pub fn get_eth_encoded_root(&self) -> H256 {
        let mut be_bytes = [0u8; 32];
        self.new_root_hash
            .into_repr()
            .write_be(be_bytes.as_mut())
            .expect("Write commit bytes");
        H256::from(be_bytes)
    }
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
        executed_tx_pub_data.resize(block_size_chunks() * 8, 0x00);

        executed_tx_pub_data
    }

    fn get_noops(&self) -> usize {
        let used_chunks = self
            .block_transactions
            .iter()
            .map(|op| {
                op.get_executed_op()
                    .map(|op| op.chunks())
                    .unwrap_or_default()
            })
            .sum::<usize>();

        block_size_chunks() - used_chunks
    }

    /// Returns eth_witness data and bytes used by each of the operations
    pub fn get_eth_witness_data(&self) -> (Vec<u8>, Vec<u64>) {
        let (eth_witness, mut used_bytes) = self.block_transactions.iter().fold(
            (Vec::new(), Vec::new()),
            |(mut eth_witness, mut used_bytes), op| {
                if let Some(witness_bytes) = op.get_eth_witness_bytes() {
                    used_bytes.push(witness_bytes.len() as u64);
                    eth_witness.extend(witness_bytes.into_iter());
                }
                (eth_witness, used_bytes)
            },
        );

        for _ in 0..self.get_noops() {
            used_bytes.push(0);
        }

        (eth_witness, used_bytes)
    }

    pub fn number_of_processed_prior_ops(&self) -> u64 {
        self.processed_priority_ops.1 - self.processed_priority_ops.0
    }
}
