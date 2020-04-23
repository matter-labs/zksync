//! Module with additional conversion methods for the storage records.
//! These methods are only needed for the `block` module, so they're kept in a
//! private module.

// Built-in deps
use std::convert::TryFrom;
// External imports
// Workspace imports
use diesel::prelude::*;
use models::{
    node::{
        block::{ExecutedPriorityOp, ExecutedTx},
        BlockNumber, FranklinOp, FranklinTx, PriorityOp,
    },
    Action, ActionType, Operation,
};
// Local imports
use crate::{
    chain::{
        block::BlockSchema,
        operations::records::{
            NewExecutedPriorityOperation, NewExecutedTransaction, StoredExecutedPriorityOperation,
            StoredExecutedTransaction, StoredOperation,
        },
        state::StateSchema,
    },
    prover::ProverSchema,
    StorageProcessor,
};

impl StoredOperation {
    pub fn into_op(self, conn: &StorageProcessor) -> QueryResult<Operation> {
        let block_number = self.block_number as BlockNumber;
        let id = Some(self.id);

        let action = if self.action_type == ActionType::COMMIT.to_string() {
            Action::Commit
        } else if self.action_type == ActionType::VERIFY.to_string() {
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

impl StoredExecutedTransaction {
    pub fn into_executed_tx(self) -> Result<ExecutedTx, failure::Error> {
        let franklin_tx: FranklinTx =
            serde_json::from_value(self.tx).expect("Unparsable FranklinTx in db");
        let franklin_op: Option<FranklinOp> =
            serde_json::from_value(self.operation).expect("Unparsable FranklinOp in db");
        Ok(ExecutedTx {
            tx: franklin_tx,
            success: self.success,
            op: franklin_op,
            fail_reason: self.fail_reason,
            block_index: self
                .block_index
                .map(|val| u32::try_from(val).expect("Invalid block index")),
            created_at: chrono::Utc::now(),
        })
    }
}

impl StoredExecutedPriorityOperation {
    pub fn into_executed(self) -> ExecutedPriorityOp {
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

impl NewExecutedPriorityOperation {
    pub fn prepare_stored_priority_op(
        exec_prior_op: ExecutedPriorityOp,
        block: BlockNumber,
    ) -> Self {
        let mut operation = serde_json::to_value(&exec_prior_op.op).unwrap();
        operation["eth_fee"] =
            serde_json::to_value(exec_prior_op.priority_op.eth_fee.to_string()).unwrap();

        let (from_account, to_account) = match exec_prior_op.op {
            FranklinOp::Deposit(deposit) => (deposit.priority_op.from, deposit.priority_op.to),
            FranklinOp::FullExit(full_exit) => {
                let eth_address = full_exit.priority_op.eth_address;
                (eth_address, eth_address)
            }
            _ => panic!(
                "Incorrect type of priority op: {:?}",
                exec_prior_op.priority_op
            ),
        };

        Self {
            block_number: i64::from(block),
            block_index: exec_prior_op.block_index as i32,
            operation,
            from_account: from_account.as_ref().to_vec(),
            to_account: to_account.as_ref().to_vec(),
            priority_op_serialid: exec_prior_op.priority_op.serial_id as i64,
            deadline_block: exec_prior_op.priority_op.deadline_block as i64,
            eth_fee: exec_prior_op.priority_op.eth_fee,
            eth_hash: exec_prior_op.priority_op.eth_hash,
        }
    }
}

impl NewExecutedTransaction {
    pub fn prepare_stored_tx(exec_tx: ExecutedTx, block: BlockNumber) -> Self {
        fn cut_prefix(input: &str) -> String {
            if input.starts_with("0x") {
                input[2..].into()
            } else if input.starts_with("sync:") {
                input[5..].into()
            } else {
                input.into()
            }
        }

        let tx = serde_json::to_value(&exec_tx.tx).expect("Cannot serialize tx");
        let operation = serde_json::to_value(&exec_tx.op).expect("Cannot serialize operation");

        let (from_account_hex, to_account_hex): (String, Option<String>) = match exec_tx.tx {
            FranklinTx::Withdraw(_) | FranklinTx::Transfer(_) => (
                serde_json::from_value(tx["from"].clone()).unwrap(),
                serde_json::from_value(tx["to"].clone()).unwrap(),
            ),
            FranklinTx::ChangePubKey(_) => (
                serde_json::from_value(tx["account"].clone()).unwrap(),
                serde_json::from_value(tx["newPkHash"].clone()).unwrap(),
            ),
            FranklinTx::Close(_) => (
                serde_json::from_value(tx["account"].clone()).unwrap(),
                serde_json::from_value(tx["account"].clone()).unwrap(),
            ),
        };

        let from_account: Vec<u8> = hex::decode(cut_prefix(&from_account_hex)).unwrap();
        let to_account: Option<Vec<u8>> =
            to_account_hex.map(|value| hex::decode(cut_prefix(&value)).unwrap());

        Self {
            block_number: i64::from(block),
            tx_hash: exec_tx.tx.hash().as_ref().to_vec(),
            from_account,
            to_account,
            tx,
            operation,
            success: exec_tx.success,
            fail_reason: exec_tx.fail_reason,
            block_index: exec_tx.block_index.map(|idx| idx as i32),
            primary_account_address: exec_tx.tx.account().as_bytes().to_vec(),
            nonce: exec_tx.tx.nonce() as i64,
            created_at: exec_tx.created_at,
        }
    }
}
