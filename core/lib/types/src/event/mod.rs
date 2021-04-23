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

#[derive(Debug, Clone, Serialize)]
pub struct ZkSyncEvent {
    #[serde(skip)]
    pub id: i64,
    #[serde(flatten)]
    pub data: EventData,
}
