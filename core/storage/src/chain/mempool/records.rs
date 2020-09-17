// External imports
use chrono::{DateTime, Utc};
use sqlx::FromRow;
// Workspace imports
// Local imports

#[derive(Debug, FromRow)]
pub struct MempoolTx {
    pub id: i64,
    pub tx_hash: String,
    pub tx: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub eth_sign_data: Option<serde_json::Value>,
    pub batch_id: i64,
}
