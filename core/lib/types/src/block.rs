//! zkSync network block definition.

use super::PriorityOp;
use super::ZkSyncOp;
use super::{AccountId, BlockNumber, Fr};
use crate::SignedZkSyncTx;
use chrono::DateTime;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use zksync_basic_types::{H256, U256};
use zksync_crypto::franklin_crypto::bellman::pairing::ff::{PrimeField, PrimeFieldRepr};
use zksync_crypto::params::CHUNK_BIT_WIDTH;
use zksync_crypto::serialization::FrSerde;

/// An intermediate state of the block in the zkSync network.
/// Contains the information about (so far) executed transactions and
/// meta-information related to the block creating process.
#[derive(Clone, Debug)]
pub struct PendingBlock {
    /// Block ID.
    pub number: BlockNumber,
    /// Amount of chunks left in the block.
    pub chunks_left: usize,
    /// ID of the first unprocessed priority operation at the moment
    /// of the block initialization.
    pub unprocessed_priority_op_before: u64,
    /// Amount of processing iterations applied to the pending block.
    /// If this amount exceeds the limit configured in the server, block will be
    /// sealed even if it's not full.
    pub pending_block_iteration: usize,
    /// List of successfully executed operations.
    pub success_operations: Vec<ExecutedOperations>,
    /// Lit of failed operations.
    pub failed_txs: Vec<ExecutedTx>,
}

/// Executed L2 transaction.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutedTx {
    pub signed_tx: SignedZkSyncTx,
    pub success: bool,
    pub op: Option<ZkSyncOp>,
    pub fail_reason: Option<String>,
    pub block_index: Option<u32>,
    pub created_at: DateTime<Utc>,
    pub batch_id: Option<i64>,
}

/// Executed L1 priority operation.
/// Unlike L2 transactions, L1 priority operations cannot fail in L2.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutedPriorityOp {
    pub priority_op: PriorityOp,
    pub op: ZkSyncOp,
    pub block_index: u32,
    pub created_at: DateTime<Utc>,
}

/// Representation of executed operation, which can be either L1 or L2.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ExecutedOperations {
    Tx(Box<ExecutedTx>),
    PriorityOp(Box<ExecutedPriorityOp>),
}

impl ExecutedOperations {
    /// Returns the `ZkSyncOp` object associated with the operation, if any.
    pub fn get_executed_op(&self) -> Option<&ZkSyncOp> {
        match self {
            ExecutedOperations::Tx(exec_tx) => exec_tx.op.as_ref(),
            ExecutedOperations::PriorityOp(exec_op) => Some(&exec_op.op),
        }
    }

    /// Attempts to get the executed L1 transaction.
    pub fn get_executed_tx(&self) -> Option<&ExecutedTx> {
        match self {
            ExecutedOperations::Tx(exec_tx) => Some(exec_tx),
            ExecutedOperations::PriorityOp(_) => None,
        }
    }

    /// Returns the public data required for the Ethereum smart contract to commit the operation.
    pub fn get_eth_public_data(&self) -> Vec<u8> {
        self.get_executed_op()
            .map(ZkSyncOp::public_data)
            .unwrap_or_default()
    }

    /// Gets the witness required for the Ethereum smart contract.
    /// Unlike public data, some operations may not have a witness.
    pub fn get_eth_witness_bytes(&self) -> Option<Vec<u8>> {
        self.get_executed_op()
            .map(|op| op.eth_witness().unwrap_or_else(Vec::new))
    }

    /// Returns the list of accounts affected by the operation.
    pub fn get_updated_account_ids(&self) -> Vec<AccountId> {
        self.get_executed_op()
            .map(|op| op.get_updated_account_ids())
            .unwrap_or_else(Vec::new)
    }
}

/// zkSync network block.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    /// Block ID.
    pub block_number: BlockNumber,
    /// Chain root hash obtained after executing this block.
    #[serde(with = "FrSerde")]
    pub new_root_hash: Fr,
    /// ID of the zkSync account to which fees are collected.
    pub fee_account: AccountId,
    /// List of operations executed in the block. Includes both L1 and L2 operations.
    pub block_transactions: Vec<ExecutedOperations>,
    /// A tuple of ID of the first unprocessed priority operation before and after this block.
    pub processed_priority_ops: (u64, u64),
    /// Actual block chunks amount that will be used on contract, such that `block_chunks_sizes >= block.chunks_used()`.
    /// Server and provers may support blocks of several different sizes, and this value must be equal to one of the
    /// supported size values.
    pub block_chunks_size: usize,

    /// Gas limit to be set for the Commit Ethereum transaction.
    pub commit_gas_limit: U256,
    /// Gas limit to be set for the Verify Ethereum transaction.
    pub verify_gas_limit: U256,
}

impl Block {
    /// Creates a new `Block` object.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        block_number: BlockNumber,
        new_root_hash: Fr,
        fee_account: AccountId,
        block_transactions: Vec<ExecutedOperations>,
        processed_priority_ops: (u64, u64),
        block_chunks_size: usize,
        commit_gas_limit: U256,
        verify_gas_limit: U256,
    ) -> Self {
        Self {
            block_number,
            new_root_hash,
            fee_account,
            block_transactions,
            processed_priority_ops,
            block_chunks_size,
            commit_gas_limit,
            verify_gas_limit,
        }
    }

    /// Creates a new block, choosing the smallest supported block size which will fit
    /// all the executed transactions.
    ///
    /// # Panics
    ///
    /// Panics if there is no supported block size to fit all the transactions.
    #[allow(clippy::too_many_arguments)]
    pub fn new_from_available_block_sizes(
        block_number: BlockNumber,
        new_root_hash: Fr,
        fee_account: AccountId,
        block_transactions: Vec<ExecutedOperations>,
        processed_priority_ops: (u64, u64),
        available_block_chunks_sizes: &[usize],
        commit_gas_limit: U256,
        verify_gas_limit: U256,
    ) -> Self {
        let mut block = Self {
            block_number,
            new_root_hash,
            fee_account,
            block_transactions,
            processed_priority_ops,
            block_chunks_size: 0,
            commit_gas_limit,
            verify_gas_limit,
        };
        block.block_chunks_size = block.smallest_block_size(available_block_chunks_sizes);
        block
    }

    /// Returns the new state root hash encoded for the Ethereum smart contract.
    pub fn get_eth_encoded_root(&self) -> H256 {
        let mut be_bytes = [0u8; 32];
        self.new_root_hash
            .into_repr()
            .write_be(be_bytes.as_mut())
            .expect("Write commit bytes");
        H256::from(be_bytes)
    }

    /// Returns the public data for the Ethereum Commit operation.
    pub fn get_eth_public_data(&self) -> Vec<u8> {
        let mut executed_tx_pub_data = self
            .block_transactions
            .iter()
            .filter_map(ExecutedOperations::get_executed_op)
            .flat_map(ZkSyncOp::public_data)
            .collect::<Vec<_>>();

        // Pad block with noops.
        executed_tx_pub_data.resize(self.block_chunks_size * CHUNK_BIT_WIDTH / 8, 0x00);

        executed_tx_pub_data
    }

    /// Returns eth_witness data and data_size for each operation that has it.
    pub fn get_eth_witness_data(&self) -> (Vec<u8>, Vec<u64>) {
        let mut eth_witness = Vec::new();
        let mut used_bytes = Vec::new();

        for block_tx in &self.block_transactions {
            if let Some(franklin_op) = block_tx.get_executed_op() {
                if let Some(witness_bytes) = franklin_op.eth_witness() {
                    used_bytes.push(witness_bytes.len() as u64);
                    eth_witness.extend(witness_bytes.into_iter());
                }
            }
        }

        (eth_witness, used_bytes)
    }

    /// Returns the number of priority operations processed in this block.
    pub fn number_of_processed_prior_ops(&self) -> u64 {
        self.processed_priority_ops.1 - self.processed_priority_ops.0
    }

    fn chunks_used(&self) -> usize {
        self.block_transactions
            .iter()
            .filter_map(ExecutedOperations::get_executed_op)
            .map(ZkSyncOp::chunks)
            .sum()
    }

    fn smallest_block_size(&self, available_block_sizes: &[usize]) -> usize {
        let chunks_used = self.chunks_used();
        smallest_block_size_for_chunks(chunks_used, available_block_sizes)
    }

    /// Returns the data about withdrawals required for the Ethereum smart contract.
    pub fn get_withdrawals_data(&self) -> Vec<u8> {
        let mut withdrawals_data = Vec::new();

        for block_tx in &self.block_transactions {
            if let Some(franklin_op) = block_tx.get_executed_op() {
                if let Some(withdrawal_data) = franklin_op.withdrawal_data() {
                    withdrawals_data.extend(&withdrawal_data);
                }
            }
        }

        withdrawals_data
    }
}

/// Gets smallest block size given the list of supported chunk sizes.
pub fn smallest_block_size_for_chunks(
    chunks_used: usize,
    available_block_sizes: &[usize],
) -> usize {
    for &block_size in available_block_sizes {
        if block_size >= chunks_used {
            return block_size;
        }
    }
    panic!(
        "Provided chunks amount ({}) cannot fit in one block, maximum available size is {}",
        chunks_used,
        available_block_sizes.last().unwrap()
    );
}
