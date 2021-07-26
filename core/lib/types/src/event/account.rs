// Built-in uses
use std::convert::TryFrom;
// External uses
use bigdecimal::BigDecimal;
use num::BigInt;
use serde::{Deserialize, Serialize};
// Workspace uses
// Local uses
use crate::{
    account::{AccountUpdate, PubKeyHash},
    aggregated_operations::AggregatedActionType,
    AccountId, Nonce, TokenId,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    #[serde(skip)]
    MintNFT,
    #[serde(skip)]
    RemoveNFT,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountEvent {
    pub update_type: AccountStateChangeType,
    pub status: AccountStateChangeStatus,
    pub update_details: AccountUpdateDetails,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountUpdateDetails {
    pub account_id: AccountId,
    pub nonce: Nonce,
    pub new_pub_key_hash: Option<PubKeyHash>,
    pub token_id: Option<TokenId>,
    pub new_balance: Option<BigDecimal>,
}

impl AccountUpdateDetails {
    pub fn from_account_update(
        account_id: AccountId,
        account_update: AccountUpdate,
    ) -> Option<Self> {
        match account_update {
            AccountUpdate::Create { address: _, nonce } => Some(Self {
                account_id,
                nonce,
                new_pub_key_hash: None,
                token_id: None,
                new_balance: None,
            }),
            AccountUpdate::Delete { address: _, nonce } => Some(Self {
                account_id,
                nonce,
                new_pub_key_hash: None,
                token_id: None,
                new_balance: None,
            }),
            AccountUpdate::UpdateBalance {
                old_nonce: _,
                new_nonce,
                balance_update,
            } => Some(Self {
                account_id,
                nonce: new_nonce,
                new_pub_key_hash: None,
                token_id: Some(balance_update.0),
                new_balance: Some(BigDecimal::from(BigInt::from(balance_update.2))),
            }),
            AccountUpdate::ChangePubKeyHash {
                old_pub_key_hash: _,
                new_pub_key_hash,
                old_nonce: _,
                new_nonce,
            } => Some(Self {
                account_id,
                nonce: new_nonce,
                new_pub_key_hash: Some(new_pub_key_hash),
                token_id: None,
                new_balance: None,
            }),
            // Do not notify about minting nft's
            AccountUpdate::MintNFT { .. } | AccountUpdate::RemoveNFT { .. } => None,
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
            AccountUpdate::MintNFT { .. } => AccountStateChangeType::MintNFT,
            AccountUpdate::RemoveNFT { .. } => AccountStateChangeType::RemoveNFT,
        }
    }
}

impl TryFrom<AggregatedActionType> for AccountStateChangeStatus {
    type Error = &'static str;

    fn try_from(action_type: AggregatedActionType) -> Result<Self, Self::Error> {
        match action_type {
            AggregatedActionType::CommitBlocks => Ok(Self::Committed),
            AggregatedActionType::ExecuteBlocks => Ok(Self::Finalized),
            _ => Err("No matching account update status for the given action type"),
        }
    }
}
