use num::BigUint;
use serde::{Deserialize, Serialize};

use super::{Nonce, TokenId};
use zksync_basic_types::Address;

use super::PubKeyHash;

/// Atomic change in the account state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AccountUpdate {
    /// Create a new account.
    Create { address: Address, nonce: Nonce },
    /// Delete an existing account.
    /// Note: Currently this kind of update is not used directly in the network.
    Delete { address: Address, nonce: Nonce },
    /// Change the account balance.
    UpdateBalance {
        old_nonce: Nonce,
        new_nonce: Nonce,
        /// Tuple of (token, old_balance, new_balance)
        balance_update: (TokenId, BigUint, BigUint),
    },
    /// Change the account Public Key.
    ChangePubKeyHash {
        old_pub_key_hash: PubKeyHash,
        new_pub_key_hash: PubKeyHash,
        old_nonce: Nonce,
        new_nonce: Nonce,
    },
}

impl AccountUpdate {
    /// Generates an account update to revert current update.
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
