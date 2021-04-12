// TODO: Move to zksync_types.
pub mod account;
pub mod block;
pub mod transaction;

use self::account::AccountEvent;
use self::block::BlockEvent;
use self::transaction::TransactionEvent;

use super::records::*;

pub use super::records::EventType;

#[derive(Debug, Clone)]
pub enum EventData {
    Account(AccountEvent),
    Block(BlockEvent),
    Transaction(TransactionEvent),
}

#[derive(Debug, Clone)]
pub struct ZkSyncEvent {
    pub id: i64,
    pub data: EventData,
}

impl From<StoredEvent> for ZkSyncEvent {
    fn from(stored_event: StoredEvent) -> Self {
        let id = stored_event.id;
        let data = match &stored_event.event_type {
            EventType::Account => {
                EventData::Account(serde_json::from_value(stored_event.event_data).unwrap())
            }
            EventType::Block => {
                EventData::Block(serde_json::from_value(stored_event.event_data).unwrap())
            }
            EventType::Transaction => {
                EventData::Transaction(serde_json::from_value(stored_event.event_data).unwrap())
            }
        };
        Self { id, data }
    }
}

impl ZkSyncEvent {
    pub fn get_type(&self) -> EventType {
        match self.data {
            EventData::Account(_) => EventType::Account,
            EventData::Block(_) => EventType::Block,
            EventData::Transaction(_) => EventType::Transaction,
        }
    }
}
