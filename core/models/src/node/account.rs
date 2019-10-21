use crate::params;
use crate::primitives::GetBits;

use std::collections::HashMap;
use std::convert::TryInto;

use bigdecimal::BigDecimal;
use failure::ensure;
use ff::PrimeField;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::Engine;
use super::Fr;
use super::{AccountId, AccountUpdates, Nonce, TokenId};
use crate::circuit::account::{Balance, CircuitAccount};
use crate::circuit::utils::pub_key_hash_bytes;
use crate::merkle_tree::pedersen_hasher::BabyPedersenHasher;
use franklin_crypto::eddsa::PublicKey;

#[derive(Clone, PartialEq, Default, Eq, Hash)]
pub struct AccountAddress {
    pub data: [u8; params::FR_ADDRESS_LEN],
}

impl std::fmt::Debug for AccountAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl AccountAddress {
    pub fn zero() -> Self {
        AccountAddress {
            data: [0; params::FR_ADDRESS_LEN],
        }
    }

    pub fn to_hex(&self) -> String {
        format!("0x{}", hex::encode(&self.data))
    }

    pub fn from_hex(s: &str) -> Result<Self, failure::Error> {
        ensure!(s.starts_with("0x"), "Address should start with 0x");
        let bytes = hex::decode(&s[2..])?;
        ensure!(bytes.len() == params::FR_ADDRESS_LEN, "Size mismatch");
        Ok(AccountAddress {
            data: bytes.as_slice().try_into().unwrap(),
        })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, failure::Error> {
        ensure!(bytes.len() == params::FR_ADDRESS_LEN, "Size mismatch");
        Ok(AccountAddress {
            data: bytes.try_into().unwrap(),
        })
    }

    pub fn from_pubkey(public_key: PublicKey<Engine>) -> Self {
        let mut pk_hash =
            pub_key_hash_bytes(&public_key, &params::PEDERSEN_HASHER as &BabyPedersenHasher);
        pk_hash.reverse();
        Self::from_bytes(&pk_hash).expect("pk convert error")
    }
}

impl Serialize for AccountAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for AccountAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        String::deserialize(deserializer).and_then(|string| {
            AccountAddress::from_hex(&string).map_err(|err| Error::custom(err.to_string()))
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub address: AccountAddress,
    balances: HashMap<TokenId, BigDecimal>,
    pub nonce: Nonce,
}

impl PartialEq for Account {
    fn eq(&self, other: &Account) -> bool {
        self.get_bits_le().eq(&other.get_bits_le())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountUpdate {
    Create {
        address: AccountAddress,
        nonce: Nonce,
    },
    Delete {
        address: AccountAddress,
        nonce: Nonce,
    },
    UpdateBalance {
        old_nonce: Nonce,
        new_nonce: Nonce,
        // (token, old, new)
        balance_update: (TokenId, BigDecimal, BigDecimal),
    },
}

// TODO: Check if coding to Fr is the same as in the circuit.
impl From<Account> for CircuitAccount<super::Engine> {
    fn from(acc: Account) -> Self {
        let mut circuit_account = CircuitAccount::default();

        let balances: Vec<_> = acc
            .balances
            .iter()
            .map(|(id, b)| {
                (
                    *id,
                    Balance {
                        value: Fr::from_str(&b.to_string()).unwrap(),
                    },
                )
            })
            .collect();

        for (i, b) in balances.into_iter() {
            circuit_account.subtree.insert(u32::from(i), b);
        }

        circuit_account.nonce = Fr::from_str(&acc.nonce.to_string()).unwrap();
        circuit_account.pub_key_hash = Fr::from_hex(&acc.address.to_hex()).unwrap();
        circuit_account
    }
}

impl AccountUpdate {
    pub fn reversed_update(&self) -> Self {
        match self {
            AccountUpdate::Create { address, nonce } => AccountUpdate::Delete {
                address: address.clone(),
                nonce: *nonce,
            },
            AccountUpdate::Delete { address, nonce } => AccountUpdate::Create {
                address: address.clone(),
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
        }
    }
}

impl Default for Account {
    fn default() -> Self {
        Self {
            balances: HashMap::new(),
            nonce: 0,
            address: AccountAddress::default(),
        }
    }
}

impl GetBits for Account {
    fn get_bits_le(&self) -> Vec<bool> {
        CircuitAccount::<super::Engine>::from(self.clone()).get_bits_le()
    }
}

impl Account {
    pub fn create_account(id: AccountId, address: AccountAddress) -> (Account, AccountUpdates) {
        let mut account = Account::default();
        account.address = address;
        let updates = vec![(
            id,
            AccountUpdate::Create {
                address: account.address.clone(),
                nonce: account.nonce,
            },
        )];
        (account, updates)
    }

    pub fn get_balance(&self, token: TokenId) -> BigDecimal {
        self.balances.get(&token).cloned().unwrap_or_default()
    }

    pub fn set_balance(&mut self, token: TokenId, amount: BigDecimal) {
        self.balances.insert(token, amount);
    }

    pub fn add_balance(&mut self, token: TokenId, amount: &BigDecimal) {
        let mut balance = self.balances.remove(&token).unwrap_or_default();
        balance += amount;
        self.balances.insert(token, balance);
    }

    pub fn sub_balance(&mut self, token: TokenId, amount: &BigDecimal) {
        let mut balance = self.balances.remove(&token).unwrap_or_default();
        balance -= amount;
        self.balances.insert(token, balance);
    }

    pub fn apply_update(account: Option<Self>, update: AccountUpdate) -> Option<Self> {
        match account {
            Some(mut account) => match update {
                AccountUpdate::Delete { .. } => None,
                AccountUpdate::UpdateBalance {
                    balance_update: (token, _, amount),
                    new_nonce,
                    ..
                } => {
                    account.set_balance(token, amount);
                    account.nonce = new_nonce;
                    Some(account)
                }
                _ => {
                    error!(
                        "Incorrect update received {:?} for account {:?}",
                        update, account
                    );
                    Some(account)
                }
            },
            None => match update {
                AccountUpdate::Create { address, nonce, .. } => {
                    let mut new_account = Account::default();
                    new_account.address = address;
                    new_account.nonce = nonce;
                    Some(new_account)
                }
                _ => {
                    error!("Incorrect update received {:?} for empty account", update);
                    None
                }
            },
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::node::{apply_updates, reverse_updates, AccountMap};
    use crate::{AccountAddress, AccountUpdates};

    #[test]
    fn test_default_account() {
        let a = Account::default();
        a.get_bits_le();
    }

    #[test]
    fn test_account_update() {
        let create = AccountUpdate::Create {
            address: AccountAddress::default(),
            nonce: 1,
        };

        let bal_update = AccountUpdate::UpdateBalance {
            old_nonce: 1,
            new_nonce: 2,
            balance_update: (0, BigDecimal::from(0), BigDecimal::from(5)),
        };

        let delete = AccountUpdate::Delete {
            address: AccountAddress::default(),
            nonce: 2,
        };

        {
            {
                let mut created_account = Account::default();
                created_account.nonce = 1;
                assert_eq!(
                    Account::apply_update(None, create.clone())
                        .unwrap()
                        .get_bits_le(),
                    created_account.get_bits_le()
                );
            }

            assert!(Account::apply_update(None, bal_update.clone()).is_none());
            assert!(Account::apply_update(None, delete.clone()).is_none());
        }
        {
            assert_eq!(
                Account::apply_update(Some(Account::default()), create.clone())
                    .unwrap()
                    .get_bits_le(),
                Account::default().get_bits_le()
            );
            {
                let mut updated_account = Account::default();
                updated_account.nonce = 2;
                updated_account.set_balance(0, BigDecimal::from(5));
                assert_eq!(
                    Account::apply_update(Some(Account::default()), bal_update.clone())
                        .unwrap()
                        .get_bits_le(),
                    updated_account.get_bits_le()
                );
            }
            assert!(Account::apply_update(Some(Account::default()), delete.clone()).is_none());
        }
    }

    #[test]
    fn test_account_updates() {
        // Create two accounts: 0, 1
        // In updates -> delete 0, update balance of 1, create account 2
        // Reverse updates

        let account_map_initial = {
            let mut map = AccountMap::default();
            let mut account_0 = Account::default();
            account_0.nonce = 8;
            let mut account_1 = Account::default();
            account_1.nonce = 16;
            map.insert(0, account_0);
            map.insert(1, account_1);
            map
        };

        let account_map_updated_expected = {
            let mut map = AccountMap::default();
            let mut account_1 = Account::default();
            account_1.nonce = 17;
            account_1.set_balance(0, BigDecimal::from(256));
            map.insert(1, account_1);
            let mut account_2 = Account::default();
            account_2.nonce = 36;
            map.insert(2, account_2);
            map
        };

        let updates = {
            let mut updates = AccountUpdates::new();
            updates.push((
                0,
                AccountUpdate::Delete {
                    address: AccountAddress::default(),
                    nonce: 8,
                },
            ));
            updates.push((
                1,
                AccountUpdate::UpdateBalance {
                    old_nonce: 16,
                    new_nonce: 17,
                    balance_update: (0, BigDecimal::from(0), BigDecimal::from(256)),
                },
            ));
            updates.push((
                2,
                AccountUpdate::Create {
                    address: AccountAddress::default(),
                    nonce: 36,
                },
            ));
            updates
        };

        let account_map_updated = {
            let mut map = account_map_initial.clone();
            apply_updates(&mut map, updates.clone());
            map
        };

        assert_eq!(account_map_updated, account_map_updated_expected);

        let account_map_updated_back = {
            let mut map = account_map_updated.clone();
            let mut reversed = updates;
            reverse_updates(&mut reversed);
            apply_updates(&mut map, reversed);
            map
        };

        assert_eq!(account_map_updated_back, account_map_initial);
    }
}
