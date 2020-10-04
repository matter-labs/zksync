// Built-in deps
use std::collections::HashMap;
// External uses
// Workspace deps
use zksync_types::PriorityOp;
// Local deps
use super::{received_ops::ReceivedPriorityOp, EthBlockId};

/// Gathered state of the Ethereum network.
/// Contains information about the known token types and incoming
/// priority operations (such as `Deposit` and `FullExit`).
///
/// All the data held is intentionally made private: as it represents the
/// observed state of the contract on Ethereum, it should never be
/// "partially updated". The state is either updated completely, or not
/// updated at all.
#[derive(Debug, Default, Clone)]
pub struct ETHState {
    /// The last block of the Ethereum network known to the Ethereum watcher.
    last_ethereum_block: u64,
    /// Queue of priority operations that are accepted by Ethereum network,
    /// but not yet have enough confirmations to be processed by zkSync.
    ///
    /// Note that since these operations do not have enough confirmations,
    /// they may be not executed in the future, so this list is approximate.
    ///
    /// Keys in this HashMap are numbers of blocks with `PriorityOp`.
    unconfirmed_queue: Vec<(EthBlockId, PriorityOp)>,
    /// Queue of priority operations that passed the confirmation
    /// threshold and are waiting to be executed.
    priority_queue: HashMap<u64, ReceivedPriorityOp>,
}

impl ETHState {
    pub fn new(
        last_ethereum_block: u64,
        unconfirmed_queue: Vec<(EthBlockId, PriorityOp)>,
        priority_queue: HashMap<u64, ReceivedPriorityOp>,
    ) -> Self {
        Self {
            last_ethereum_block,
            unconfirmed_queue,
            priority_queue,
        }
    }

    pub fn last_ethereum_block(&self) -> u64 {
        self.last_ethereum_block
    }

    pub fn priority_queue(&self) -> &HashMap<u64, ReceivedPriorityOp> {
        &self.priority_queue
    }

    pub fn unconfirmed_queue(&self) -> &[(EthBlockId, PriorityOp)] {
        &self.unconfirmed_queue
    }
}
