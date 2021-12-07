// External uses
use futures::channel::oneshot;
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
