// External uses
use serde::{Deserialize, Serialize};
use serde_json::value::Value;
use sqlx::FromRow;

#[derive(sqlx::Type, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[sqlx(rename = "event_type")]
#[sqlx(rename_all = "UPPERCASE")]
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
