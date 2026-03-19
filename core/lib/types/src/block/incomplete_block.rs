//! zkSync network block definition.

use super::{AccountId, BlockNumber, ZkSyncOp};
use chrono::Utc;
use chrono::{DateTime, TimeZone};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use zksync_basic_types::U256;

use super::ExecutedOperations;

/// Sealed, but not yet completed zkSync block data.
/// This structure contains data available in the state keeper when the block is sealed,
/// but misses data to calculate the commitment (mainly, the root hash of the block).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IncompleteBlock {
    /// Block ID.
    pub block_number: BlockNumber,
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
    /// Timestamp
    pub timestamp: u64,
}

impl IncompleteBlock {
    /// Creates a new `IncompleteBlock` object.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        block_number: BlockNumber,
        fee_account: AccountId,
        block_transactions: Vec<ExecutedOperations>,
        processed_priority_ops: (u64, u64),
        block_chunks_size: usize,
        commit_gas_limit: U256,
        verify_gas_limit: U256,
        timestamp: u64,
    ) -> Self {
        Self {
            block_number,
            fee_account,
            block_transactions,
            processed_priority_ops,
            block_chunks_size,
            commit_gas_limit,
            verify_gas_limit,
            timestamp,
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
        fee_account: AccountId,
        block_transactions: Vec<ExecutedOperations>,
        processed_priority_ops: (u64, u64),
        available_block_chunks_sizes: &[usize],
        commit_gas_limit: U256,
        verify_gas_limit: U256,
        timestamp: u64,
    ) -> Self {
        let mut block = Self {
            block_number,
            fee_account,
            block_transactions,
            processed_priority_ops,
            block_chunks_size: 0,
            commit_gas_limit,
            verify_gas_limit,
            timestamp,
        };
        block.block_chunks_size = block.smallest_block_size(available_block_chunks_sizes);
        block
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

    pub fn timestamp_utc(&self) -> DateTime<Utc> {
        Utc.timestamp_opt(self.timestamp as i64, 0).unwrap()
    }

    pub fn elapsed(&self) -> Duration {
        (Utc::now() - self.timestamp_utc())
            .to_std()
            .unwrap_or_default()
    }
}

/// Gets smallest block size given the list of supported chunk sizes.
fn smallest_block_size_for_chunks(chunks_used: usize, available_block_sizes: &[usize]) -> usize {
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
