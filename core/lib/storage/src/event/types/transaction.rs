// Built-in uses
use std::cell::Cell;
// External uses
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
// Workspace uses
use zksync_basic_types::{AccountId, BlockNumber};
use zksync_types::block::ExecutedOperations;
// Local uses

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    Committed,
    Rejected,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Deserialize)]
pub enum TransactionType {
    Transfer,
    Withdraw,
    ChangePubKey,
    ForcedExit,
    FullExit,
    Deposit,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionEvent {
    pub tx_hash: String,
    pub account_id: i64,
    pub token_id: i32,
    pub block_number: i64,
    pub tx: serde_json::Value,
    pub status: TransactionStatus,
    pub fail_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    /// This field is only used for filtering.
    #[serde(skip)]
    tx_type: Cell<Option<TransactionType>>,
}

impl TransactionEvent {
    pub fn from_executed_operation(
        op: &ExecutedOperations,
        block: BlockNumber,
        account_id: AccountId,
    ) -> Self {
        match op {
            ExecutedOperations::Tx(exec_tx) => Self {
                tx_hash: exec_tx.signed_tx.tx.hash().to_string(),
                account_id: i64::from(*account_id),
                token_id: i32::from(*exec_tx.signed_tx.token_id()),
                block_number: i64::from(*block),
                tx: serde_json::to_value(exec_tx.signed_tx.tx.clone()).unwrap(),
                status: if exec_tx.success {
                    TransactionStatus::Committed
                } else {
                    TransactionStatus::Rejected
                },
                fail_reason: exec_tx.fail_reason.clone(),
                created_at: exec_tx.created_at,
                tx_type: Cell::default(),
            },
            ExecutedOperations::PriorityOp(exec_prior_op) => Self {
                tx_hash: exec_prior_op.priority_op.eth_hash.to_string(),
                account_id: i64::from(*account_id),
                token_id: i32::from(*exec_prior_op.priority_op.data.token_id()),
                block_number: i64::from(*block),
                tx: serde_json::to_value(&exec_prior_op.op.clone()).unwrap(),
                status: TransactionStatus::Committed,
                fail_reason: None,
                created_at: exec_prior_op.created_at,
                tx_type: Cell::default(),
            },
        }
    }

    pub fn tx_type(&self) -> TransactionType {
        match self.tx_type.get() {
            Some(tx_type) => tx_type,
            None => {
                let tx_type = serde_json::from_value(self.tx["type"].clone()).unwrap();
                self.tx_type.set(Some(tx_type));
                tx_type
            }
        }
    }
}
