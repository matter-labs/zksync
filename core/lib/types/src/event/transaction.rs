// Built-in uses
use std::convert::TryFrom;
// External uses
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
// Workspace uses
// Local uses
use crate::{block::ExecutedOperations, AccountId, BlockNumber};

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
    /// TODO: move to another type after removing deserializing
    #[serde(skip)]
    tx_type: Option<TransactionType>,
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
                tx_type: None,
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
                tx_type: None,
            },
        }
    }

    pub fn tx_type(&self) -> TransactionType {
        self.tx_type.unwrap()
    }
}

impl TryFrom<serde_json::Value> for TransactionEvent {
    type Error = serde_json::Error;

    fn try_from(value: serde_json::Value) -> Result<Self, Self::Error> {
        let mut tx_event: TransactionEvent = serde_json::from_value(value)?;
        let tx_type = serde_json::from_value(tx_event.tx["type"].clone())?;
        tx_event.tx_type = Some(tx_type);
        Ok(tx_event)
    }
}
