// External uses
use serde::{Deserialize, Serialize};
use serde_json::value::Value;
use sqlx::FromRow;

#[derive(sqlx::Type, Debug, Clone, Serialize, Deserialize)]
#[sqlx(rename = "event_type")]
#[sqlx(rename_all = "UPPERCASE")]
pub enum EventType {
    Account,
    Block,
    Transaction,
}

#[derive(FromRow, Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    id: i64,
    event_type: EventType,
    event_data: Value,
    is_processed: bool,
}
