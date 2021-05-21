// Built-in uses
// External uses
use chrono::{DateTime, Utc};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
// Workspace uses
// Local uses
use super::account::AccountStateChangeStatus;
use crate::{block::ExecutedOperations, AccountId, BlockNumber, TokenId};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    Committed,
    Finalized,
    Rejected,
}

/// All possible types of operations in the zkSync network.
/// Deserialized from the `tx` field of the [TransactionEvent].
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Deserialize)]
pub enum TransactionType {
    Transfer,
    Withdraw,
    WithdrawNFT,
    MintNFT,
    Swap,
    ChangePubKey,
    ForcedExit,
    FullExit,
    Deposit,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionEvent {
    pub tx_hash: String,
    pub account_id: AccountId,
    pub token_id: TokenId,
    pub block_number: BlockNumber,
    pub tx: serde_json::Value,
    pub status: TransactionStatus,
    pub fail_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    /// This field is lazily initialized and only used for filtering.
    /// Since the event is shared among server worker threads, it has
    /// to implement [std::marker::Sync], which is not the case for [std::cell::Cell].
    #[serde(skip)]
    pub(crate) tx_type: OnceCell<TransactionType>,
}

impl TransactionEvent {
    pub fn from_executed_operation(
        op: ExecutedOperations,
        block_number: BlockNumber,
        account_id: AccountId,
        status: TransactionStatus,
    ) -> Self {
        match op {
            ExecutedOperations::Tx(exec_tx) => Self {
                tx_hash: exec_tx.signed_tx.tx.hash().to_string(),
                account_id,
                token_id: exec_tx.signed_tx.token_id(),
                block_number,
                tx: serde_json::to_value(exec_tx.signed_tx.tx).unwrap(),
                status,
                fail_reason: exec_tx.fail_reason,
                created_at: exec_tx.created_at,
                tx_type: OnceCell::default(),
            },
            ExecutedOperations::PriorityOp(exec_prior_op) => Self {
                tx_hash: format!("{:#x}", exec_prior_op.priority_op.eth_hash),
                account_id,
                token_id: exec_prior_op.priority_op.data.token_id(),
                block_number,
                tx: serde_json::to_value(&exec_prior_op.op).unwrap(),
                status,
                fail_reason: None,
                created_at: exec_prior_op.created_at,
                tx_type: OnceCell::default(),
            },
        }
    }

    pub fn tx_type(&self) -> TransactionType {
        *self
            .tx_type
            .get_or_init(|| serde_json::from_value(self.tx["type"].clone()).unwrap())
    }
}

impl From<AccountStateChangeStatus> for TransactionStatus {
    fn from(status: AccountStateChangeStatus) -> Self {
        match status {
            AccountStateChangeStatus::Committed => Self::Committed,
            AccountStateChangeStatus::Finalized => Self::Finalized,
        }
    }
}
