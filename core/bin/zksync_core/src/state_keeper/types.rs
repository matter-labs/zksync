// External uses
use futures::channel::oneshot;
// Workspace uses
use zksync_types::{Account, AccountId, Address};
// Local uses
use crate::{mempool::ProposedBlock, state_keeper::init_params::ZkSyncStateInitParams};

pub enum StateKeeperRequest {
    GetAccount(Address, oneshot::Sender<Option<(AccountId, Account)>>),
    GetPendingBlockTimestamp(oneshot::Sender<u64>),
    GetLastUnprocessedPriorityOp(oneshot::Sender<u64>),
    ExecuteMiniBlock(ProposedBlock),
    SealBlock,
    GetCurrentState(oneshot::Sender<ZkSyncStateInitParams>),
}
