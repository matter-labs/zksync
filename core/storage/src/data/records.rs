//! This module contains the structures that represent the contents
//! of the tables. Each structure is associated with one of the tables
//! used in project and is used to interact with the database.

// External imports
use serde_json::value::Value;
// Workspace imports
use models::node::{AccountId, BlockNumber, FranklinOp};
// Local imports
use crate::schema::*;

#[derive(Debug, Clone, Queryable, QueryableByName)]
#[table_name = "rollup_ops"]
pub struct StoredFranklinOp {
    pub id: i32,
    pub block_num: i64,
    pub operation: Value,
    pub fee_account: i64,
}

impl StoredFranklinOp {
    pub fn into_franklin_op(self) -> FranklinOp {
        serde_json::from_value(self.operation).expect("Unparsable FranklinOp in db")
    }
}
#[derive(Debug, Insertable)]
#[table_name = "rollup_ops"]
pub struct NewFranklinOp {
    pub block_num: i64,
    pub operation: Value,
    pub fee_account: i64,
}

impl NewFranklinOp {
    pub fn prepare_stored_op(
        franklin_op: &FranklinOp,
        block: BlockNumber,
        fee_account: AccountId,
    ) -> Self {
        Self {
            block_num: i64::from(block),
            operation: serde_json::to_value(franklin_op.clone()).unwrap(),
            fee_account: i64::from(fee_account),
        }
    }
}
