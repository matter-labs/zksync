// Built-in uses
// External uses
use serde::Serialize;
// Workspace uses
// Local uses
use self::account::AccountEvent;
use self::block::BlockEvent;
use self::transaction::TransactionEvent;

pub mod account;
pub mod block;
pub mod transaction;

#[derive(Debug, Clone, Serialize)]
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
