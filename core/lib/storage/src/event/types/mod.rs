// TODO: Move to zksync_types.
use serde::{Deserialize, Serialize};

pub mod account;
pub mod block;
pub mod transaction;

use self::account::AccountEvent;
use self::block::BlockEvent;
use self::transaction::TransactionEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ZkSyncEvent {
    Account(AccountEvent),
    Block(BlockEvent),
    Transaction(TransactionEvent),
}
