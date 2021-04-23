use std::convert::TryFrom;

// Built-in uses
// External uses
use serde::{Deserialize, Serialize};
use serde_json::value::Value;
use sqlx::FromRow;
// Workspace uses
use zksync_types::event::{transaction::TransactionEvent, EventData, ZkSyncEvent};
// Local uses

#[derive(sqlx::Type, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[sqlx(rename = "event_type")]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Account,
    Block,
    Transaction,
}

#[derive(FromRow, Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    pub id: i64,
    pub event_type: EventType,
    pub event_data: Value,
    pub is_processed: bool,
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

pub fn get_event_type(event: &ZkSyncEvent) -> EventType {
    match event.data {
        EventData::Account(_) => EventType::Account,
        EventData::Block(_) => EventType::Block,
        EventData::Transaction(_) => EventType::Transaction,
    }
}
