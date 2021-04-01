// Built-in uses
// External uses
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
// Workspace uses
use zksync_basic_types::BlockNumber;
use zksync_types::block::ExecutedOperations;
// Local uses

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    Committed,
    Rejected,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionEvent {
    pub tx_hash: String,
    pub account_id: i64,
    pub token_id: i32,
    pub block_number: i64,
    pub tx: Value,
    pub status: TransactionStatus,
    pub fail_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl TransactionEvent {
    pub fn from_executed_operation(
        op: &ExecutedOperations,
        block: BlockNumber,
    ) -> anyhow::Result<Self> {
        Ok(match op {
            ExecutedOperations::Tx(exec_tx) => Self {
                tx_hash: exec_tx.signed_tx.tx.hash().to_string(),
                account_id: i64::from(*exec_tx.signed_tx.account_id()?),
                token_id: i32::from(*exec_tx.signed_tx.token_id()),
                block_number: i64::from(*block),
                tx: serde_json::to_value(exec_tx.signed_tx.tx.clone())?,
                status: if exec_tx.success {
                    TransactionStatus::Committed
                } else {
                    TransactionStatus::Rejected
                },
                fail_reason: exec_tx.fail_reason.clone(),
                created_at: exec_tx.created_at.clone(),
            },
            ExecutedOperations::PriorityOp(exec_prior_op) => {
                // We have to fetch the account id for the `Deposit` operation.
                Self {
                    tx_hash: exec_prior_op.priority_op.eth_hash.to_string(),
                    account_id: 0i64,
                    token_id: i32::from(*exec_prior_op.priority_op.data.token_id()),
                    block_number: i64::from(*block),
                    tx: serde_json::to_value(&exec_prior_op.op.clone())?,
                    status: TransactionStatus::Committed,
                    fail_reason: None,
                    created_at: exec_prior_op.created_at.clone(),
                }
            }
        })
    }
}
