use crate::circuit;
use crate::plasma::params::{self, TOTAL_TOKENS};
use crate::plasma::{AccountId, AccountUpdates, Nonce, TokenAmount, TokenId};
use crate::primitives::GetBits;
use crate::{Engine, Fr, PublicKey};
use bigdecimal::BigDecimal;
use sapling_crypto::jubjub::{edwards, Unknown};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, Eq, Hash)]
pub struct AccountAddress {
    pub data: [u8; 27],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub address: AccountAddress,
    balances: Vec<BigDecimal>,
    pub nonce: Nonce,
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
            balances: vec![BigDecimal::from(0); TOTAL_TOKENS],
            nonce: 0,
            address: AccountAddress::default(),
        }
    }
}

impl GetBits for Account {
    fn get_bits_le(&self) -> Vec<bool> {
        use bitvec::prelude::*;
        use ff::{PrimeField, PrimeFieldRepr};

        unimplemented!()

        //        // TODO: (Drogan) use circuit here.
        //        let mut bytes = Vec::new();
        //        for token in 0..TOTAL_TOKENS {
        //            bytes.extend_from_slice(&self.get_balance(token as TokenId).to_le_bytes());
        //        }
        //        bytes.extend_from_slice(&self.nonce.to_le_bytes());
        //        self.public_key_x.into_repr().write_le(&mut bytes).unwrap();
        //        self.public_key_y.into_repr().write_le(&mut bytes).unwrap();
        //
        //        BitVec::<LittleEndian, u8>::from_slice(&bytes)
        //            .into_iter()
        //            .collect()

        // let mut leaf_content = Vec::new();
        // leaf_content.extend(self.balance.get_bits_le_fixed(params::BALANCE_BIT_WIDTH));
        // leaf_content.extend(self.nonce.get_bits_le_fixed(params::NONCE_BIT_WIDTH));
        // leaf_content.extend(self.pub_x.get_bits_le_fixed(params::FR_BIT_WIDTH));
        // leaf_content.extend(self.pub_y.get_bits_le_fixed(params::FR_BIT_WIDTH));
        // leaf_content
    }
}

impl Account {
    pub fn create_account(id: AccountId, address: AccountAddress) -> (Account, AccountUpdates) {
        let mut account = Account::default();
        account.address = address;
        let mut updates = vec![(
            id,
            AccountUpdate::Create {
                address: account.address.clone(),
                nonce: account.nonce,
            },
        )];
        (account, updates)
    }

    pub fn get_balance(&self, token: TokenId) -> BigDecimal {
        self.balances[token as usize].clone()
    }

    pub fn set_balance(&mut self, token: TokenId, amount: BigDecimal) {
        self.balances[token as usize] = amount;
    }

    pub fn add_balance(&mut self, token: TokenId, amount: &BigDecimal) {
        self.balances[token as usize] += amount;
    }

    pub fn sub_balance(&mut self, token: TokenId, amount: &BigDecimal) {
        self.balances[token as usize] -= amount;
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
    //    use super::*;
    //    use crate::plasma::{apply_updates, reverse_updates};
    //    use crate::{AccountMap, AccountUpdates};
    //
    //    #[test]
    //    fn test_default_account() {
    //        let a = Account::default();
    //        a.get_bits_le();
    //    }
    //
    //    #[test]
    //    fn test_account_update() {
    //        let create = AccountUpdate::Create {
    //            public_key_x: Fr::default(),
    //            public_key_y: Fr::default(),
    //            nonce: 1,
    //        };
    //
    //        let bal_update = AccountUpdate::UpdateBalance {
    //            old_nonce: 1,
    //            new_nonce: 2,
    //            balance_update: (0, BigDecimal::from(0), BigDecimal::from(5)),
    //        };
    //
    //        let delete = AccountUpdate::Delete {
    //            public_key_x: Fr::default(),
    //            public_key_y: Fr::default(),
    //            nonce: 2,
    //        };
    //
    //        {
    //            {
    //                let mut created_account = Account::default();
    //                created_account.nonce = 1;
    //                assert_eq!(
    //                    Account::apply_update(None, create.clone())
    //                        .unwrap()
    //                        .get_bits_le(),
    //                    created_account.get_bits_le()
    //                );
    //            }
    //
    //            assert!(Account::apply_update(None, bal_update.clone()).is_none());
    //            assert!(Account::apply_update(None, delete.clone()).is_none());
    //        }
    //        {
    //            assert_eq!(
    //                Account::apply_update(Some(Account::default()), create.clone())
    //                    .unwrap()
    //                    .get_bits_le(),
    //                Account::default().get_bits_le()
    //            );
    //            {
    //                let mut updated_account = Account::default();
    //                updated_account.nonce = 2;
    //                updated_account.set_balance(0, &BigDecimal::from(5));
    //                assert_eq!(
    //                    Account::apply_update(Some(Account::default()), bal_update.clone())
    //                        .unwrap()
    //                        .get_bits_le(),
    //                    updated_account.get_bits_le()
    //                );
    //            }
    //            assert!(Account::apply_update(Some(Account::default()), delete.clone()).is_none());
    //        }
    //    }
    //
    //    #[test]
    //    fn test_account_updates() {
    //        // Create two accounts: 0, 1
    //        // In updates -> delete 0, update balance of 1, create account 2
    //        // Reverse updates
    //
    //        let account_map_initial = {
    //            let mut map = AccountMap::default();
    //            let mut account_0 = Account::default();
    //            account_0.nonce = 8;
    //            let mut account_1 = Account::default();
    //            account_1.nonce = 16;
    //            map.insert(0, account_0);
    //            map.insert(1, account_1);
    //            map
    //        };
    //
    //        let account_map_updated_expected = {
    //            let mut map = AccountMap::default();
    //            let mut account_1 = Account::default();
    //            account_1.nonce = 17;
    //            account_1.set_balance(0, &BigDecimal::from(256));
    //            map.insert(1, account_1);
    //            let mut account_2 = Account::default();
    //            account_2.nonce = 36;
    //            map.insert(2, account_2);
    //            map
    //        };
    //
    //        let updates = {
    //            let mut updates = AccountUpdates::new();
    //            updates.push((
    //                0,
    //                AccountUpdate::Delete {
    //                    public_key_y: Fr::default(),
    //                    public_key_x: Fr::default(),
    //                    nonce: 8,
    //                },
    //            ));
    //            updates.push((
    //                1,
    //                AccountUpdate::UpdateBalance {
    //                    old_nonce: 16,
    //                    new_nonce: 17,
    //                    balance_update: (0, BigDecimal::from(0), BigDecimal::from(256)),
    //                },
    //            ));
    //            updates.push((
    //                2,
    //                AccountUpdate::Create {
    //                    public_key_y: Fr::default(),
    //                    public_key_x: Fr::default(),
    //                    nonce: 36,
    //                },
    //            ));
    //            updates
    //        };
    //
    //        let account_map_updated = {
    //            let mut map = account_map_initial.clone();
    //            apply_updates(&mut map, updates.clone());
    //            map
    //        };
    //
    //        assert_eq!(account_map_updated, account_map_updated_expected);
    //
    //        let account_map_updated_back = {
    //            let mut map = account_map_updated.clone();
    //            let mut reversed = updates;
    //            reverse_updates(&mut reversed);
    //            apply_updates(&mut map, reversed);
    //            map
    //        };
    //
    //        assert_eq!(account_map_updated_back, account_map_initial);
    //    }
}
