// Built-in uses

// External uses
use num::BigInt;
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
// Workspace uses
use zksync_basic_types::AccountId;
use zksync_types::account::{AccountUpdate, PubKeyHash};
// Local uses
use crate::diff::StorageAccountDiff;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountStateChangeStatus {
    Committed,
    Finalized,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountStateChangeType {
    Create,
    Delete,
    UpdateBalance,
    ChangePubKeyHash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountEvent {
    pub update_type: AccountStateChangeType,
    pub status: AccountStateChangeStatus,
    pub account_update_details: AccountUpdateDetails,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountUpdateDetails {
    pub account_id: i64,
    pub nonce: i64,
    pub new_pub_key_hash: Option<PubKeyHash>,
    pub token_id: Option<i32>,
    pub new_balance: Option<BigDecimal>,
}

impl AccountUpdateDetails {
    pub fn from_account_update(account_id: AccountId, account_update: &AccountUpdate) -> Self {
        match account_update {
            AccountUpdate::Create { address: _, nonce } => Self {
                account_id: i64::from(*account_id),
                nonce: i64::from(**nonce),
                new_pub_key_hash: None,
                token_id: None,
                new_balance: None,
            },
            AccountUpdate::Delete { address: _, nonce } => Self {
                account_id: i64::from(*account_id),
                nonce: i64::from(**nonce),
                new_pub_key_hash: None,
                token_id: None,
                new_balance: None,
            },
            AccountUpdate::UpdateBalance {
                old_nonce: _,
                new_nonce,
                balance_update,
            } => Self {
                account_id: i64::from(*account_id),
                nonce: i64::from(**new_nonce),
                new_pub_key_hash: None,
                token_id: Some(i32::from(*balance_update.0)),
                new_balance: Some(BigDecimal::from(BigInt::from(balance_update.2.clone()))),
            },
            AccountUpdate::ChangePubKeyHash {
                old_pub_key_hash: _,
                new_pub_key_hash,
                old_nonce: _,
                new_nonce,
            } => Self {
                account_id: i64::from(*account_id),
                nonce: i64::from(**new_nonce),
                new_pub_key_hash: Some(*new_pub_key_hash),
                token_id: None,
                new_balance: None,
            },
        }
    }
}

impl From<&AccountUpdate> for AccountStateChangeType {
    fn from(account_update: &AccountUpdate) -> Self {
        match account_update {
            AccountUpdate::Create { .. } => AccountStateChangeType::Create,
            AccountUpdate::Delete { .. } => AccountStateChangeType::Delete,
            AccountUpdate::UpdateBalance { .. } => AccountStateChangeType::UpdateBalance,
            AccountUpdate::ChangePubKeyHash { .. } => AccountStateChangeType::ChangePubKeyHash,
        }
    }
}

impl From<&StorageAccountDiff> for AccountStateChangeType {
    fn from(account_update: &StorageAccountDiff) -> Self {
        match account_update {
            StorageAccountDiff::BalanceUpdate(_) => AccountStateChangeType::UpdateBalance,
            StorageAccountDiff::Create(_) => AccountStateChangeType::Create,
            StorageAccountDiff::Delete(_) => AccountStateChangeType::Delete,
            StorageAccountDiff::ChangePubKey(_) => AccountStateChangeType::ChangePubKeyHash,
        }
    }
}

impl From<&StorageAccountDiff> for AccountUpdateDetails {
    fn from(account_diff: &StorageAccountDiff) -> Self {
        match account_diff {
            StorageAccountDiff::BalanceUpdate(update) => Self {
                account_id: update.account_id,
                nonce: update.new_nonce,
                new_pub_key_hash: None,
                token_id: Some(update.coin_id),
                new_balance: Some(update.new_balance.clone()),
            },
            StorageAccountDiff::Create(update) => Self {
                account_id: update.account_id,
                nonce: update.nonce,
                new_pub_key_hash: None,
                token_id: None,
                new_balance: None,
            },
            StorageAccountDiff::Delete(update) => Self {
                account_id: update.account_id,
                nonce: update.nonce,
                new_pub_key_hash: None,
                token_id: None,
                new_balance: None,
            },
            StorageAccountDiff::ChangePubKey(update) => Self {
                account_id: update.account_id,
                nonce: update.new_nonce,
                new_pub_key_hash: Some(
                    PubKeyHash::from_bytes(update.new_pubkey_hash.as_slice()).unwrap(),
                ),
                token_id: None,
                new_balance: None,
            },
        }
    }
}
