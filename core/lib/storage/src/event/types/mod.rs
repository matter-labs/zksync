// TODO: Move to zksync_types.
pub mod account;
pub mod block;
pub mod transaction;

use std::convert::TryFrom;

use self::account::AccountEvent;
use self::block::BlockEvent;
use self::transaction::TransactionEvent;

use super::records::*;
use serde::Serialize;

pub use super::records::EventType;

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
                EventData::Transaction(TransactionEvent::try_from(stored_event.event_data).unwrap())
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
