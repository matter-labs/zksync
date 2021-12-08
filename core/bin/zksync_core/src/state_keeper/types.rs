// External uses
use futures::channel::oneshot;
use itertools::Itertools;
// Workspace uses
use zksync_types::{Account, AccountId, Address};
// Local uses
use crate::{mempool::ProposedBlock, state_keeper::init_params::ZkSyncStateInitParams};

#[derive(Debug)]
pub enum StateKeeperRequest {
    GetAccount(Address, oneshot::Sender<Option<(AccountId, Account)>>),
    GetPendingBlockTimestamp(oneshot::Sender<u64>),
    GetLastUnprocessedPriorityOp(oneshot::Sender<u64>),
    ExecuteMiniBlock(ProposedBlock),
    SealBlock,
    GetCurrentState(oneshot::Sender<ZkSyncStateInitParams>),
}

#[derive(Debug)]
pub enum ApplyOutcome<T> {
    Included(T),
    NotIncluded,
}

impl<T> ApplyOutcome<T> {
    pub fn assert_included(&self, msg: &str) {
        if matches!(self, Self::NotIncluded) {
            panic!("{}", msg)
        }
    }

    #[cfg(test)]
    pub fn included(&self) -> bool {
        matches!(self, Self::Included(_))
    }

    #[cfg(test)]
    pub fn not_included(&self) -> bool {
        !self.included()
    }
}

/// Constant configuration parameters needed by state keeper to work.
#[derive(Debug)]
pub(super) struct StateKeeperConfig {
    pub(super) fee_account_id: AccountId,
    pub(super) available_block_chunk_sizes: Vec<usize>,
    pub(super) max_miniblock_iterations: usize,
    pub(super) fast_miniblock_iterations: usize,
    max_block_size: usize,
}

impl StateKeeperConfig {
    pub(super) fn new(
        fee_account_id: AccountId,
        available_block_chunk_sizes: Vec<usize>,
        max_miniblock_iterations: usize,
        fast_miniblock_iterations: usize,
    ) -> Self {
        // Ensure that available block chunk sizes are sorted and not empty.
        assert!(
            !available_block_chunk_sizes.is_empty(),
            "Block chunk sizes are empty"
        );
        let is_sorted = available_block_chunk_sizes
            .iter()
            .tuple_windows()
            .all(|(a, b)| a < b);
        assert!(
            is_sorted,
            "Block chunk sizes are not in order: {:?}",
            available_block_chunk_sizes
        );

        // Maximum size that block can have.
        let max_block_size = *available_block_chunk_sizes.iter().max().unwrap();

        Self {
            fee_account_id,
            available_block_chunk_sizes,
            max_miniblock_iterations,
            fast_miniblock_iterations,
            max_block_size,
        }
    }

    pub(super) fn max_block_size(&self) -> usize {
        self.max_block_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks that config can be created if provided values are correct.
    #[test]
    fn create_config() {
        let config = StateKeeperConfig::new(AccountId(0), vec![1, 2, 3], 10, 20);
        assert_eq!(config.max_block_size, 3);
    }

    /// Checks that if chunk sizes are not in order, it will panic.
    #[test]
    #[should_panic(expected = "Block chunk sizes are not in order")]
    fn config_chunks_out_of_order() {
        let incorrect_chunks = vec![3, 1, 2];
        let _config = StateKeeperConfig::new(AccountId(0), incorrect_chunks, 10, 20);
    }

    /// Checks that if chunk sizes are empty, it will panic.
    #[test]
    #[should_panic(expected = "Block chunk sizes are empty")]
    fn config_chunks_empty() {
        let incorrect_chunks = vec![];
        let _config = StateKeeperConfig::new(AccountId(0), incorrect_chunks, 10, 20);
    }
}
