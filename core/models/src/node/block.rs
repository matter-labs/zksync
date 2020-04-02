use super::FranklinOp;
use super::FranklinTx;
use super::PriorityOp;
use super::{AccountId, BlockNumber, Fr};
use crate::franklin_crypto::bellman::pairing::ff::{PrimeField, PrimeFieldRepr};
use crate::params::{block_chunk_sizes, max_block_chunk_size};
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
            .map(FranklinOp::public_data)
            .unwrap_or_default()
    }

    pub fn get_eth_witness_bytes(&self) -> Option<Vec<u8>> {
        self.get_executed_op().map(FranklinOp::eth_witness)
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
            .filter_map(ExecutedOperations::get_executed_op)
            .flat_map(FranklinOp::public_data)
            .collect::<Vec<_>>();

        // Pad block with noops.
        executed_tx_pub_data.resize(self.smallest_block_size() * 8, 0x00);

        executed_tx_pub_data
    }

    fn get_noops(&self) -> usize {
        self.smallest_block_size() - self.chunks_used()
    }

    /// Returns eth_witness data and bytes used by each of the operations (except transfer: since there is no need for the eth witness, it is processed separately to reduce gas consumption)
    pub fn get_eth_witness_data(&self) -> (Vec<u8>, Vec<u64>) {
        let mut eth_witness = Vec::new();
        let mut used_bytes = Vec::new();

        for block_tx in &self.block_transactions {
            if let Some(franklin_op) = block_tx.get_executed_op() {
                if let FranklinOp::Transfer(_) =  franklin_op { // skip transfer operations
                    continue;
                }
                let witness_bytes = franklin_op.eth_witness();
                used_bytes.push(witness_bytes.len() as u64);
                eth_witness.extend(witness_bytes.into_iter());
            }
        }

        for _ in 0..self.get_noops() {
            used_bytes.push(0);
        }

        (eth_witness, used_bytes)
    }

    pub fn number_of_processed_prior_ops(&self) -> u64 {
        self.processed_priority_ops.1 - self.processed_priority_ops.0
    }

    pub fn chunks_used(&self) -> usize {
        self.block_transactions
            .iter()
            .filter_map(ExecutedOperations::get_executed_op)
            .map(FranklinOp::chunks)
            .sum()
    }

    pub fn smallest_block_size(&self) -> usize {
        let chunks_used = self.chunks_used();
        Self::smallest_block_size_for_chunks(chunks_used)
    }

    pub fn smallest_block_size_for_chunks(chunks_used: usize) -> usize {
        for &block_size in block_chunk_sizes() {
            if block_size >= chunks_used {
                return block_size;
            }
        }
        panic!(
            "Provided chunks amount ({}) cannot fit in one block, maximum available size is {}",
            chunks_used,
            max_block_chunk_size()
        );
    }
}
