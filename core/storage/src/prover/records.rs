// External imports
use chrono::prelude::*;
use diesel::sql_types::*;
use serde_derive::{Deserialize, Serialize};
// Workspace imports
// Local imports
use crate::schema::*;

#[derive(Debug, Insertable, Queryable, QueryableByName)]
#[table_name = "active_provers"]
pub struct ActiveProver {
    pub id: i32,
    pub worker: String,
    pub created_at: NaiveDateTime,
    pub stopped_at: Option<NaiveDateTime>,
    pub block_size: i64,
}

#[derive(Debug, Insertable, Queryable, QueryableByName)]
#[table_name = "proofs"]
pub struct NewProof {
    pub block_number: i64,
    pub proof: serde_json::Value,
}

#[derive(Debug, Insertable, Queryable, QueryableByName)]
#[table_name = "proofs"]
pub struct StoredProof {
    pub block_number: i64,
    pub proof: serde_json::Value,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Insertable, Queryable, QueryableByName)]
#[table_name = "multiblock_proofs"]
pub struct NewMultiblockProof {
    pub block_from: i64,
    pub block_to: i64,
    pub proof: serde_json::Value,
}

#[derive(Debug, Insertable, Queryable, QueryableByName)]
#[table_name = "multiblock_proofs"]
pub struct StoredMultiblockProof {
    pub id: i32,
    pub block_from: i64,
    pub block_to: i64,
    pub proof: serde_json::Value,
    pub created_at: NaiveDateTime,
}

// Every time before a prover worker starts generating the proof, a prover run is recorded for monitoring purposes
#[derive(Debug, Clone, Insertable, Queryable, QueryableByName, Serialize, Deserialize)]
#[table_name = "prover_runs"]
pub struct ProverRun {
    pub id: i32,
    pub block_number: i64,
    pub worker: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

// Every time before a prover worker starts generating the multiblock proof, a prover run is recorded for monitoring purposes
#[derive(Debug, Clone, Insertable, Queryable, QueryableByName, Serialize, Deserialize)]
#[table_name = "prover_multiblock_runs"]
pub struct ProverMultiblockRun {
    pub id: i32,
    pub block_number_from: i64,
    pub block_number_to: i64,
    pub worker: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, QueryableByName)]
pub struct IntegerNumber {
    #[sql_type = "BigInt"]
    pub integer_value: i64,
}

#[derive(Debug, Serialize, Deserialize, QueryableByName, PartialEq)]
pub struct MultiproofBlockItem {
    #[sql_type = "BigInt"]
    pub block_number: i64,

    #[sql_type = "Bool"]
    pub blocks_batch_timeout_passed: bool,

    #[sql_type = "Bool"]
    pub multiblock_already_generated: bool,
}
