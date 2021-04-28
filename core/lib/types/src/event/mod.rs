// Built-in uses
// External uses
use serde::Serialize;
// Workspace uses
// Local uses
use self::{account::AccountEvent, block::BlockEvent, transaction::TransactionEvent};

pub mod account;
pub mod block;
pub mod transaction;

pub mod test_data;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventData {
    Account(AccountEvent),
    Block(BlockEvent),
    Transaction(TransactionEvent),
}

// An event that happened in the zkSync network.
// Only created by the `storage`.
#[derive(Debug, Clone, Serialize)]
pub struct ZkSyncEvent {
    // Id of the event. This value is equal to
    // the id of the corresponding row in the database.
    pub id: i64,
    pub data: EventData,
    // TODO: Add a block_number field (ZKS-653).
}
