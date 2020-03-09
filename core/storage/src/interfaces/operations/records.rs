// External imports
use bigdecimal::BigDecimal;
use chrono::prelude::*;
use diesel::prelude::*;
use serde_json::value::Value;
// Workspace imports
use models::node::block::ExecutedPriorityOp;
use models::node::{AccountId, BlockNumber, FranklinOp, PriorityOp};
use models::{Action, ActionType, Operation};
// Local imports
use crate::interfaces::prover::ProverInterface;
use crate::schema::*;

// TODO this module should not know about storage processor.
use crate::StorageProcessor;

#[derive(Debug, Insertable)]
#[table_name = "executed_priority_operations"]
pub struct NewExecutedPriorityOperation {
    pub block_number: i64,
    pub block_index: i32,
    pub operation: Value,
    pub priority_op_serialid: i64,
    pub deadline_block: i64,
    pub eth_fee: BigDecimal,
    pub eth_hash: Vec<u8>,
}

impl NewExecutedPriorityOperation {
    pub fn prepare_stored_priority_op(
        exec_prior_op: &ExecutedPriorityOp,
        block: BlockNumber,
    ) -> Self {
        Self {
            block_number: i64::from(block),
            block_index: exec_prior_op.block_index as i32,
            operation: serde_json::to_value(&exec_prior_op.op).unwrap(),
            priority_op_serialid: exec_prior_op.priority_op.serial_id as i64,
            deadline_block: exec_prior_op.priority_op.deadline_block as i64,
            eth_fee: exec_prior_op.priority_op.eth_fee.clone(),
            eth_hash: exec_prior_op.priority_op.eth_hash.clone(),
        }
    }
}

#[derive(Debug, Insertable)]
#[table_name = "operations"]
pub struct NewOperation {
    pub block_number: i64,
    pub action_type: String,
}

#[derive(Debug, Clone, Queryable, QueryableByName)]
#[table_name = "operations"]
pub struct StoredOperation {
    pub id: i64,
    pub block_number: i64,
    pub action_type: String,
    pub created_at: NaiveDateTime,
    pub confirmed: bool,
}

impl StoredOperation {
    pub fn into_op(self, conn: &StorageProcessor) -> QueryResult<Operation> {
        let block_number = self.block_number as BlockNumber;
        let id = Some(self.id);

        let action = if self.action_type == ActionType::COMMIT.to_string() {
            Action::Commit
        } else if self.action_type == ActionType::VERIFY.to_string() {
            // verify
            let proof = Box::new(conn.load_proof(block_number)?);
            Action::Verify { proof }
        } else {
            unreachable!("Incorrect action type in db");
        };

        let block = conn
            .get_block(block_number)?
            .expect("Block for action does not exist");
        let accounts_updated = conn.load_state_diff_for_block(block_number)?;
        Ok(Operation {
            id,
            action,
            block,
            accounts_updated,
        })
    }
}

#[derive(Debug, Queryable, QueryableByName)]
#[table_name = "executed_priority_operations"]
pub struct StoredExecutedPriorityOperation {
    pub id: i32,
    pub block_number: i64,
    pub block_index: i32,
    pub operation: Value,
    pub priority_op_serialid: i64,
    pub deadline_block: i64,
    pub eth_fee: BigDecimal,
    pub eth_hash: Vec<u8>,
}

impl Into<ExecutedPriorityOp> for StoredExecutedPriorityOperation {
    fn into(self) -> ExecutedPriorityOp {
        let franklin_op: FranklinOp =
            serde_json::from_value(self.operation).expect("Unparsable priority op in db");
        ExecutedPriorityOp {
            priority_op: PriorityOp {
                serial_id: self.priority_op_serialid as u64,
                data: franklin_op
                    .try_get_priority_op()
                    .expect("FranklinOp should have priority op"),
                deadline_block: self.deadline_block as u64,
                eth_fee: self.eth_fee,
                eth_hash: self.eth_hash,
            },
            op: franklin_op,
            block_index: self.block_index as u32,
        }
    }
}

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
