use zksync_crypto::{
    circuit::{
        account::{Balance, CircuitAccount, CircuitBalanceTree},
        utils::eth_address_to_fr,
    },
    franklin_crypto::bellman::pairing::ff::Field,
    pairing::ff::PrimeField,
    params::{MIN_NFT_TOKEN_ID, NFT_STORAGE_ACCOUNT_ADDRESS, NFT_TOKEN_ID},
    primitives::GetBits,
    Engine, Fr,
};

use crate::hasher::{verify_accounts_equal, CustomMerkleTree, BALANCE_TREE_11, BALANCE_TREE_32};
use num::BigUint;
use serde::{Deserialize, Serialize};
use std::fs;
use std::{collections::HashMap, convert::TryInto};
use zksync_types::{account::Account, Address, Nonce, PubKeyHash, TokenId};
use zksync_utils::BigUintSerdeAsRadix10Str;

pub fn get_balance_tree(depth: usize) -> CircuitBalanceTree {
    match depth {
        11 => BALANCE_TREE_11.clone(),
        32 => BALANCE_TREE_32.clone(),
        _ => panic!("Depth {} is not supported", depth),
    }
}

// Unfortunately we need to reimplement the structs for CircuitAccount for different tree depths
macro_rules! custom_circuit_account {
    ($(#[$attr:meta])* $name:ident, $balance_tree:literal) => {
        $(#[$attr])*
        pub struct $name(pub CircuitAccount<Engine>);

        impl Default for $name {
            fn default() -> Self {
                let subtree = get_balance_tree($balance_tree);
                let circuit_account = CircuitAccount {
                    nonce: Fr::zero(),
                    pub_key_hash: Fr::zero(),
                    address: Fr::zero(),
                    subtree
                };

                Self(circuit_account)
            }
        }

        impl GetBits for $name {
            fn get_bits_le(&self) -> Vec<bool> {
                self.0.get_bits_le()
            }
        }

        impl CircuitAccountWrapper for $name {
            fn from_account(account: Account) -> Self {
                let mut circuit_account = Self::default().0;

                let balances: Vec<_> = account
                    .get_nonzero_balances()
                    .iter()
                    .map(|(token_id, balance)| {
                        (
                            *token_id,
                            Balance {
                                value: Fr::from_str(&balance.0.to_string()).unwrap(),
                            },
                        )
                    })
                    .collect();

                for (token_id, balance) in balances.into_iter() {
                    circuit_account.subtree.insert(*token_id, balance);
                }

                circuit_account.nonce = Fr::from_str(&account.nonce.to_string()).unwrap();
                circuit_account.pub_key_hash = account.pub_key_hash.to_fr();
                circuit_account.address = eth_address_to_fr(&account.address);

                Self(circuit_account)
            }

            fn get_inner(&self) -> CircuitAccount<Engine> {
                self.0.clone()
            }
        }

    };
}

custom_circuit_account!(CircuitAccountDepth11, 11);

custom_circuit_account!(CircuitAccountDepth32, 32);

pub trait CircuitAccountWrapper: Sync + Default + GetBits {
    fn from_account(account: Account) -> Self;
    fn get_inner(&self) -> CircuitAccount<Engine>;
}

#[derive(Debug, Deserialize)]
pub struct StorageAccount {
    pub id: i64,
    pub last_block: i64,
    pub nonce: i64,
    pub address: String,
    pub pubkey_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageBalance {
    pub account_id: i64,
    pub coin_id: i32,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub balance: BigUint,
}

impl TryInto<Account> for StorageAccount {
    type Error = anyhow::Error;

    fn try_into(self) -> anyhow::Result<Account> {
        let pub_key_hash_bytes = hex::decode(&self.pubkey_hash)?;
        let pub_key_hash = PubKeyHash::from_bytes(&pub_key_hash_bytes)?;

        let address_bytes = hex::decode(&self.address)?;
        let address = Address::from_slice(&address_bytes);

        let nonce = Nonce(self.nonce as u32);

        let mut result = Account::default_with_address(&address);
        result.nonce = nonce;
        result.pub_key_hash = pub_key_hash;

        Ok(result)
    }
}

pub fn read_accounts(
    path_to_accounts: String,
    path_to_balances: String,
) -> anyhow::Result<Vec<(i64, Account)>> {
    let accounts_content = fs::read_to_string(path_to_accounts)?;
    // \\x is a technical symbol added by Postgres to indicate hex
    let accounts_content = accounts_content.replace(r#"\\x"#, "");

    let balances_content = fs::read_to_string(path_to_balances)?;

    let stored_accounts: Vec<StorageAccount> = serde_json::from_str(&accounts_content)?;
    let stored_balances: Vec<StorageBalance> = serde_json::from_str(&balances_content)?;

    let mut account_map: HashMap<i64, Account> = HashMap::new();
    for stored_account in stored_accounts {
        account_map.insert(stored_account.id, stored_account.try_into()?);
    }

    for stored_balance in stored_balances {
        let account = account_map.get_mut(&stored_balance.account_id).unwrap();
        let balance: BigUint = stored_balance.balance.to_string().parse().unwrap();

        account.set_balance(TokenId(stored_balance.coin_id as u32), balance);
    }

    let accounts: Vec<(i64, Account)> = account_map.drain().collect();

    Ok(accounts)
}

pub fn verify_empty<T: CircuitAccountWrapper>(
    index: u32,
    tree: &CustomMerkleTree<T>,
) -> anyhow::Result<()> {
    let account = tree.get(index);
    match account {
        Some(inner) => {
            let zero_account = T::default();
            verify_accounts_equal(index, &zero_account, inner)?;
            Ok(())
        }
        None => Ok(()),
    }
}

pub fn get_nft_account() -> Account {
    let mut nft_account = Account::default_with_address(&NFT_STORAGE_ACCOUNT_ADDRESS);
    nft_account.set_balance(NFT_TOKEN_ID, BigUint::from(MIN_NFT_TOKEN_ID));

    nft_account
}

pub fn get_nft_circuit_account() -> CircuitAccountDepth32 {
    let nft_account = get_nft_account();

    CircuitAccountDepth32::from_account(nft_account)
}
