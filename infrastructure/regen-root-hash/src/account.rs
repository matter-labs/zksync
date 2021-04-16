use zksync_crypto::{
    circuit::{
        account::{Balance, CircuitAccount, CircuitBalanceTree},
        utils::eth_address_to_fr,
    },
    pairing::ff::PrimeField,
    Engine, Fr,
};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

use num::BigUint;

use franklin_crypto::bellman::pairing::ff::Field;
use zksync_types::{account::Account, Address, Nonce, PubKeyHash, TokenId};

/*

Even though the original library already implements
CircuitAccount with account subtree depth of 11,
here it is still reimplemented in the same way as 32 to
make sure that the implementation is correct

*/

pub trait Rehashable {
    fn from_account(account: Account, balance_tree: &CircuitBalanceTree) -> Self;
}

impl Rehashable for CircuitAccount<Engine> {
    fn from_account(account: Account, balance_tree: &CircuitBalanceTree) -> Self {
        let mut circuit_account = Self {
            nonce: Fr::zero(),
            pub_key_hash: Fr::zero(),
            address: Fr::zero(),
            subtree: balance_tree.clone(),
        };

        let balances: Vec<_> = account
            .get_nonzero_balances()
            .iter()
            .map(|(id, b)| {
                (
                    *id,
                    Balance {
                        value: Fr::from_str(&b.0.to_string()).unwrap(),
                    },
                )
            })
            .collect();

        for (i, b) in balances.into_iter() {
            circuit_account.subtree.insert(u32::from(*i), b);
        }

        circuit_account.nonce = Fr::from_str(&account.nonce.to_string()).unwrap();
        circuit_account.pub_key_hash = account.pub_key_hash.to_fr();
        circuit_account.address = eth_address_to_fr(&account.address);
        circuit_account
    }
}

#[derive(Deserialize)]
pub struct StorageAccount {
    pub id: i64,
    pub last_block: i64,
    pub nonce: i64,
    pub address: String,
    pub pubkey_hash: String,
}

#[derive(Serialize, Deserialize)]
pub struct StorageBalance {
    pub account_id: i64,
    pub coin_id: i32,
    pub balance: serde_json::Number,
}

fn storage_account_to_account(account: &StorageAccount) -> anyhow::Result<Account> {
    let pub_key_hash_bytes = hex::decode(&account.pubkey_hash)?;
    let pub_key_hash = PubKeyHash::from_bytes(&pub_key_hash_bytes)?;

    let address_bytes = hex::decode(&account.address)?;
    let address = Address::from_slice(&address_bytes);

    let nonce = Nonce(account.nonce as u32);

    let mut result = Account::default_with_address(&address);
    result.nonce = nonce;
    result.pub_key_hash = pub_key_hash;

    Ok(result)
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

    let mut account_map = HashMap::new();
    for stored_account in stored_accounts {
        let account = storage_account_to_account(&stored_account)?;
        account_map.insert(stored_account.id, account);
    }

    for stored_balance in stored_balances {
        let account = account_map.get_mut(&stored_balance.account_id).unwrap();
        let balance: BigUint = stored_balance.balance.to_string().parse().unwrap();

        account.set_balance(TokenId(stored_balance.coin_id as u16), balance);
    }

    let accounts: Vec<(i64, Account)> = account_map.drain().collect();

    Ok(accounts)
}
