// Built-in deps
use std::cmp::Ordering;
// External imports
use num::bigint::ToBigInt;
// Workspace imports
use zksync_types::{
    event::account::{AccountStateChangeType, AccountUpdateDetails},
    AccountId, AccountUpdate, Address, Nonce, PubKeyHash, TokenId,
};
// Local imports
use crate::chain::account::records::*;

/// `StorageAccoundDiff` is a enum that combines all the possible
/// changes that can be applied to account, which includes:
///
/// - Creation of the new account.
/// - Removing of the existing account.
/// - Changing balance of the account.
/// - Changing the public key of the account.
///
/// This enum allows one to process account updates in a generic way.
#[derive(Debug)]
pub enum StorageAccountDiff {
    BalanceUpdate(StorageAccountUpdate),
    Create(StorageAccountCreation),
    Delete(StorageAccountCreation),
    ChangePubKey(StorageAccountPubkeyUpdate),
}

impl From<StorageAccountUpdate> for StorageAccountDiff {
    fn from(update: StorageAccountUpdate) -> Self {
        StorageAccountDiff::BalanceUpdate(update)
    }
}

impl From<StorageAccountCreation> for StorageAccountDiff {
    fn from(create: StorageAccountCreation) -> Self {
        if create.is_create {
            StorageAccountDiff::Create(create)
        } else {
            StorageAccountDiff::Delete(create)
        }
    }
}

impl From<StorageAccountPubkeyUpdate> for StorageAccountDiff {
    fn from(update: StorageAccountPubkeyUpdate) -> Self {
        StorageAccountDiff::ChangePubKey(update)
    }
}

impl From<StorageAccountDiff> for (AccountId, AccountUpdate) {
    fn from(val: StorageAccountDiff) -> Self {
        match val {
            StorageAccountDiff::BalanceUpdate(upd) => {
                let old_balance = upd.old_balance.to_bigint().unwrap();
                let old_balance = old_balance.to_biguint().unwrap();

                let new_balance = upd.new_balance.to_bigint().unwrap();
                let new_balance = new_balance.to_biguint().unwrap();

                (
                    AccountId(upd.account_id as u32),
                    AccountUpdate::UpdateBalance {
                        old_nonce: Nonce(upd.old_nonce as u32),
                        new_nonce: Nonce(upd.new_nonce as u32),
                        balance_update: (TokenId(upd.coin_id as u16), old_balance, new_balance),
                    },
                )
            }
            StorageAccountDiff::Create(upd) => (
                AccountId(upd.account_id as u32),
                AccountUpdate::Create {
                    nonce: Nonce(upd.nonce as u32),
                    address: Address::from_slice(&upd.address.as_slice()),
                },
            ),
            StorageAccountDiff::Delete(upd) => (
                AccountId(upd.account_id as u32),
                AccountUpdate::Delete {
                    nonce: Nonce(upd.nonce as u32),
                    address: Address::from_slice(&upd.address.as_slice()),
                },
            ),
            StorageAccountDiff::ChangePubKey(upd) => (
                AccountId(upd.account_id as u32),
                AccountUpdate::ChangePubKeyHash {
                    old_nonce: Nonce(upd.old_nonce as u32),
                    new_nonce: Nonce(upd.new_nonce as u32),
                    old_pub_key_hash: PubKeyHash::from_bytes(&upd.old_pubkey_hash)
                        .expect("PubkeyHash update from db deserialize"),
                    new_pub_key_hash: PubKeyHash::from_bytes(&upd.new_pubkey_hash)
                        .expect("PubkeyHash update from db deserialize"),
                },
            ),
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

impl StorageAccountDiff {
    /// Compares updates by `block number` then by `update_order_id` (which is number within block).
    pub fn cmp_order(&self, other: &Self) -> Ordering {
        self.block_number()
            .cmp(&other.block_number())
            .then(self.update_order_id().cmp(&other.update_order_id()))
    }

    /// Returns the index of the operation within block.
    pub fn update_order_id(&self) -> i32 {
        match self {
            StorageAccountDiff::BalanceUpdate(StorageAccountUpdate {
                update_order_id, ..
            }) => *update_order_id,
            StorageAccountDiff::Create(StorageAccountCreation {
                update_order_id, ..
            }) => *update_order_id,
            StorageAccountDiff::Delete(StorageAccountCreation {
                update_order_id, ..
            }) => *update_order_id,
            StorageAccountDiff::ChangePubKey(StorageAccountPubkeyUpdate {
                update_order_id,
                ..
            }) => *update_order_id,
        }
    }

    /// Returns the block index to which the operation belongs.
    pub fn block_number(&self) -> i64 {
        *match self {
            StorageAccountDiff::BalanceUpdate(StorageAccountUpdate { block_number, .. }) => {
                block_number
            }
            StorageAccountDiff::Create(StorageAccountCreation { block_number, .. }) => block_number,
            StorageAccountDiff::Delete(StorageAccountCreation { block_number, .. }) => block_number,
            StorageAccountDiff::ChangePubKey(StorageAccountPubkeyUpdate {
                block_number, ..
            }) => block_number,
        }
    }
}
