// Built-in deps
use std::cmp::Ordering;
// External imports
use web3::types::Address;
// Workspace imports
use models::node::PubKeyHash;
use models::node::{AccountUpdate, TokenId};
// Local imports
use crate::chain::account::records::*;

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

impl Into<(u32, AccountUpdate)> for StorageAccountDiff {
    fn into(self) -> (u32, AccountUpdate) {
        match self {
            StorageAccountDiff::BalanceUpdate(upd) => (
                upd.account_id as u32,
                AccountUpdate::UpdateBalance {
                    old_nonce: upd.old_nonce as u32,
                    new_nonce: upd.new_nonce as u32,
                    balance_update: (upd.coin_id as TokenId, upd.old_balance, upd.new_balance),
                },
            ),
            StorageAccountDiff::Create(upd) => (
                upd.account_id as u32,
                AccountUpdate::Create {
                    nonce: upd.nonce as u32,
                    address: Address::from_slice(&upd.address.as_slice()),
                },
            ),
            StorageAccountDiff::Delete(upd) => (
                upd.account_id as u32,
                AccountUpdate::Delete {
                    nonce: upd.nonce as u32,
                    address: Address::from_slice(&upd.address.as_slice()),
                },
            ),
            StorageAccountDiff::ChangePubKey(upd) => (
                upd.account_id as u32,
                AccountUpdate::ChangePubKeyHash {
                    old_nonce: upd.old_nonce as u32,
                    new_nonce: upd.new_nonce as u32,
                    old_pub_key_hash: PubKeyHash::from_bytes(&upd.old_pubkey_hash)
                        .expect("PubkeyHash update from db deserialzie"),
                    new_pub_key_hash: PubKeyHash::from_bytes(&upd.new_pubkey_hash)
                        .expect("PubkeyHash update from db deserialzie"),
                },
            ),
        }
    }
}

impl StorageAccountDiff {
    pub fn update_order_id(&self) -> i32 {
        *match self {
            StorageAccountDiff::BalanceUpdate(StorageAccountUpdate {
                update_order_id, ..
            }) => update_order_id,
            StorageAccountDiff::Create(StorageAccountCreation {
                update_order_id, ..
            }) => update_order_id,
            StorageAccountDiff::Delete(StorageAccountCreation {
                update_order_id, ..
            }) => update_order_id,
            StorageAccountDiff::ChangePubKey(StorageAccountPubkeyUpdate {
                update_order_id,
                ..
            }) => update_order_id,
        }
    }

    /// Compares updates by `block number` then by `update_order_id(number within block)`.
    pub fn cmp_order(&self, other: &Self) -> Ordering {
        self.block_number()
            .cmp(&other.block_number())
            .then(self.update_order_id().cmp(&other.update_order_id()))
    }

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
