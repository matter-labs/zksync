use anyhow::Context;
use std::collections::HashMap;
use web3::types::Address;

use crate::types::{MerkleTreeLeaf, StorageAccount, StorageBalance, StorageToken};

fn csv_reader(path: &str) -> anyhow::Result<csv::Reader<std::io::BufReader<std::fs::File>>> {
    let file = std::fs::File::open(path)?;
    let reader = csv::Reader::from_reader(std::io::BufReader::new(file));
    Ok(reader)
}

/// Loads account balances from a CSV file.
/// Groups balances by account_id.
///
/// # Arguments
/// * `path` - Path to the CSV file containing balance records
///
/// # Returns
/// A HashMap mapping account_id to a vector of StorageBalance records
pub(crate) fn load_balances(path: &str) -> anyhow::Result<HashMap<u32, Vec<StorageBalance>>> {
    let mut balances = HashMap::new();
    for line in csv_reader(path)
        .with_context(|| format!("Unable to open file at {path}"))?
        .deserialize()
    {
        let balance: StorageBalance = line.with_context(|| format!("Malformed line in {path}"))?;
        balances
            .entry(balance.account_id)
            .or_insert(vec![])
            .push(balance);
    }
    println!(
        "Loaded balances for {} accounts from {}",
        balances.len(),
        path
    );
    Ok(balances)
}

/// Loads token information from a CSV file.
/// Maps token IDs to their Ethereum addresses.
///
/// # Arguments
/// * `path` - Path to the CSV file containing token records
///
/// # Returns
/// A TokenId to Address mapping
pub fn load_tokens(path: &str) -> anyhow::Result<HashMap<u64, Address>> {
    let mut tokens = HashMap::new();
    for line in csv_reader(path)
        .with_context(|| format!("Unable to open file at {path}"))?
        .deserialize()
    {
        let token: StorageToken = line?;
        tokens.insert(token.id as u64, token.address);
    }
    println!("Loaded {} tokens from {}", tokens.len(), path);
    Ok(tokens)
}

/// Loads account information from a CSV file.
///
/// # Arguments
/// * `path` - Path to the CSV file containing account records
///
/// # Returns
/// A mapping of account_id to Full Account
pub(crate) fn load_accounts(path: &str) -> anyhow::Result<HashMap<u32, StorageAccount>> {
    let mut accounts = HashMap::new();
    for line in csv_reader(path)
        .with_context(|| format!("Unable to open file at {path}"))?
        .deserialize()
    {
        let account: StorageAccount = line?;
        accounts.insert(account.id, account);
    }
    println!("Loaded {} accounts from {}", accounts.len(), path);
    Ok(accounts)
}

/// Loads Merkle tree leaves from a CSV file.
///
/// # Arguments
/// * `path` - Path to the CSV file containing the leaves
///
/// # Returns
/// Leaves mapped by (token_address, account_address)
pub fn load_keccak_merkle_leaves(
    path: &str,
) -> anyhow::Result<HashMap<(Address, Address), MerkleTreeLeaf>> {
    let mut leaves = HashMap::new();
    for line in csv_reader(path)
        .with_context(|| format!("Unable to open file at {path}"))?
        .deserialize()
    {
        let leaf: MerkleTreeLeaf = line?;
        leaves.insert((leaf.token_address, leaf.account_address), leaf);
    }
    Ok(leaves)
}
