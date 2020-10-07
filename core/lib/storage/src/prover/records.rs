// External imports
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
// Workspace imports
// Local imports

#[derive(Debug, FromRow)]
pub struct ActiveProver {
    pub id: i32,
    pub worker: String,
    pub created_at: DateTime<Utc>,
    pub stopped_at: Option<DateTime<Utc>>,
    pub block_size: i64,
}

#[derive(Debug, FromRow)]
pub struct NewProof {
    pub block_number: i64,
    pub proof: serde_json::Value,
}

#[derive(Debug, FromRow)]
pub struct StoredProof {
    pub block_number: i64,
    pub proof: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

// Every time before a prover worker starts generating the proof, a prover run is recorded for monitoring purposes
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ProverRun {
    pub id: i32,
    pub block_number: i64,
    pub worker: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub struct IntegerNumber {
    pub integer_value: i64,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct StorageBlockWitness {
    pub block: i64,
    pub witness: String,
}
