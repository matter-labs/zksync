use num::BigUint;
use serde::{Deserialize, Serialize};

use super::{Nonce, TokenId};
use zksync_basic_types::Address;

use super::PubKeyHash;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountUpdate {
    Create {
        address: Address,
        nonce: Nonce,
    },
    Delete {
        address: Address,
        nonce: Nonce,
    },
    UpdateBalance {
        old_nonce: Nonce,
        new_nonce: Nonce,
        // (token, old, new)
        balance_update: (TokenId, BigUint, BigUint),
    },
    ChangePubKeyHash {
        old_pub_key_hash: PubKeyHash,
        new_pub_key_hash: PubKeyHash,
        old_nonce: Nonce,
        new_nonce: Nonce,
    },
}

impl AccountUpdate {
    pub fn reversed_update(&self) -> Self {
        match self {
            AccountUpdate::Create { address, nonce } => AccountUpdate::Delete {
                address: *address,
                nonce: *nonce,
            },
            AccountUpdate::Delete { address, nonce } => AccountUpdate::Create {
                address: *address,
                nonce: *nonce,
            },
            AccountUpdate::UpdateBalance {
                old_nonce,
                new_nonce,
                balance_update,
            } => AccountUpdate::UpdateBalance {
                old_nonce: *new_nonce,
                new_nonce: *old_nonce,
                balance_update: (
                    balance_update.0,
                    balance_update.2.clone(),
                    balance_update.1.clone(),
                ),
            },
            AccountUpdate::ChangePubKeyHash {
                old_pub_key_hash,
                new_pub_key_hash,
                old_nonce,
                new_nonce,
            } => AccountUpdate::ChangePubKeyHash {
                old_pub_key_hash: new_pub_key_hash.clone(),
                new_pub_key_hash: old_pub_key_hash.clone(),
                old_nonce: *new_nonce,
                new_nonce: *old_nonce,
            },
        }
    }
}
