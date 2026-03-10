use std::{collections::HashMap, fs, str::FromStr};

use anyhow::Context;
use bigdecimal::BigDecimal;
use num_bigint::ToBigInt;
use web3::types::Address;
use zksync_crypto::{
    Fr, merkle_tree::parallel_smt::SparseMerkleTreeSerializableCacheBN256,
    params::account_tree_depth,
};
use zksync_types::{Account, AccountId, AccountMap, AccountTree, Nonce, PubKeyHash, TokenId};

use crate::{
    consts::INTERNALS_FILE,
    csv_utils::{load_accounts, load_balances},
    types::{StorageAccount, StorageBalance},
};

pub fn restore_zksync_tree_from_files(accounts: &str, balances: &str) -> anyhow::Result<()> {
    println!("Restoring ZKSYNC Merkle tree from provided CSV files...");
    let stored_accounts = load_accounts(accounts)?;
    let stored_balances = load_balances(balances)?;
    let account_map = restore_account_map(stored_accounts, stored_balances)?;
    let account_tree = restore_tree(account_map);
    println!("Account tree restored successfully.");
    let root_hash = calculate_root_hash(&account_tree);
    println!(
        "Restored tree root hash: 0x{root_hash} \n You can validate this root hash on the ZKSYNC contract.",
    );
    Ok(())
}

/// Restores an Account from stored account and balance data.
/// Sets up the account's balances, nonce, address, and public key hash.
/// # Returns
/// A tuple of (AccountId, Account) for the restored account
fn restore_account(
    stored_account: &StorageAccount,
    stored_balances: &[StorageBalance],
) -> (AccountId, Account) {
    let mut account = Account::default();
    account.nonce = Nonce(stored_account.nonce);
    account.address = Address::from_str(&stored_account.address).expect("Correct address");
    account.pub_key_hash = PubKeyHash::from_hex(&stored_account.pubkey_hash)
        .expect("db stored pubkey hash deserialize");
    for b in stored_balances.iter() {
        assert_eq!(b.account_id, stored_account.id);
        let balance = BigDecimal::from_str(&b.balance)
            .unwrap()
            .to_bigint()
            .unwrap()
            .to_biguint()
            .unwrap();
        account.set_balance(TokenId(b.coin_id), balance);
    }
    (AccountId(stored_account.id), account)
}

/// Restores the AccountMap from CSV files.
pub(crate) fn restore_account_map(
    stored_accounts: HashMap<u32, StorageAccount>,
    stored_balances: HashMap<u32, Vec<StorageBalance>>,
) -> anyhow::Result<AccountMap> {
    let mut account_map = AccountMap::default();
    for (account_id, stored_account) in stored_accounts {
        let balances = stored_balances.get(&stored_account.id).context(
            "Account has no balances. Please check the consistency of the provided files",
        )?;
        assert_eq!(account_id, stored_account.id);
        let (account_id, account) = restore_account(&stored_account, balances);
        account_map.insert(account_id, account);
    }
    Ok(account_map)
}

/// Restores the account tree from an account map.
/// Inserts all accounts into the tree and creates a mapping from addresses to account IDs.
/// # Returns
/// An AccountTree with all accounts inserted
/// # Note
/// It has a side effect of loading internals from a file if it exists.
pub fn restore_tree(account_map: AccountMap) -> AccountTree {
    let mut tree = AccountTree::new(account_tree_depth());
    for (account_id, account) in account_map.into_iter() {
        tree.insert(account_id.0, account);
    }
    if let Ok(cache) = fs::read(INTERNALS_FILE) {
        let cache = SparseMerkleTreeSerializableCacheBN256::decode_bincode(&cache);
        tree.set_internals(cache);
    }

    tree
}

/// Calculates the root hash of the account tree and saves internals to a file.
/// # Returns
/// A string representation of the root hash
///
/// # Note
/// It has a side effect of saving internals to a file.
fn calculate_root_hash(tree: &AccountTree) -> Fr {
    println!("Calculating root hash ...");
    let root_hash = tree.root_hash();
    let internals = tree.get_internals();
    let content = internals.encode_bincode();
    if let Err(e) = fs::write(INTERNALS_FILE, content) {
        println!(
            "Failed to save internals to file: {}. \n The next restore will be slower",
            e
        );
    }
    root_hash
}
