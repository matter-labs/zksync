// External imports
use bigdecimal::BigDecimal;
use chrono::prelude::*;
use diesel::prelude::*;
use serde_json::value::Value;
// Workspace imports
use models::node::block::{ExecutedPriorityOp, ExecutedTx};
use models::node::{AccountId, BlockNumber, FranklinOp, FranklinTx, PriorityOp};
use models::{Action, ActionType, Operation};
// Local imports
use crate::schema::*;

// TODO this module should not know about storage processor and interfaces.
use crate::interfaces::{
    block::BlockSchema, operations_ext::records::ReadTx, prover::ProverSchema, state::StateSchema,
};
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
            let proof = Box::new(ProverSchema(&conn).load_proof(block_number)?);
            Action::Verify { proof }
        } else {
            unreachable!("Incorrect action type in db");
        };

        let block = BlockSchema(&conn)
            .get_block(block_number)?
            .expect("Block for action does not exist");
        let accounts_updated = StateSchema(&conn).load_state_diff_for_block(block_number)?;
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

#[derive(Debug, Queryable, QueryableByName)]
#[table_name = "executed_transactions"]
pub struct StoredExecutedTransaction {
    pub id: i32,
    pub block_number: i64,
    pub tx_hash: Vec<u8>,
    pub operation: Option<Value>,
    pub success: bool,
    pub fail_reason: Option<String>,
    pub block_index: Option<i32>,
}

impl StoredExecutedTransaction {
    pub fn into_executed_tx(self, stored_tx: Option<ReadTx>) -> Result<ExecutedTx, failure::Error> {
        if let Some(op) = self.operation {
            let franklin_op: FranklinOp =
                serde_json::from_value(op).expect("Unparsable FranklinOp in db");
            Ok(ExecutedTx {
                tx: franklin_op
                    .try_get_tx()
                    .expect("FranklinOp should not have tx"),
                success: true,
                op: Some(franklin_op),
                fail_reason: None,
                block_index: Some(self.block_index.expect("Block idx should be set") as u32),
            })
        } else if let Some(stored_tx) = stored_tx {
            let tx: FranklinTx = serde_json::from_value(stored_tx.tx).expect("Unparsable tx in db");
            Ok(ExecutedTx {
                tx,
                success: false,
                op: None,
                fail_reason: self.fail_reason,
                block_index: None,
            })
        } else {
            failure::bail!("Unsuccessful tx was lost from db.");
        }
    }
}

#[derive(Debug, Insertable)]
#[table_name = "executed_transactions"]
pub struct NewExecutedTransaction {
    pub block_number: i64,
    pub tx_hash: Vec<u8>,
    pub operation: Option<Value>,
    pub success: bool,
    pub fail_reason: Option<String>,
    pub block_index: Option<i32>,
}

impl NewExecutedTransaction {
    pub fn prepare_stored_tx(exec_tx: &ExecutedTx, block: BlockNumber) -> Self {
        Self {
            block_number: i64::from(block),
            tx_hash: exec_tx.tx.hash().as_ref().to_vec(),
            operation: exec_tx.op.clone().map(|o| serde_json::to_value(o).unwrap()),
            success: exec_tx.success,
            fail_reason: exec_tx.fail_reason.clone(),
            block_index: exec_tx.block_index.map(|idx| idx as i32),
        }
    }
}
