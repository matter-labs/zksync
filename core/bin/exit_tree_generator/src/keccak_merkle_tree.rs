use std::{collections::HashMap, str::FromStr};

use anyhow::Context;
use rayon::prelude::*;
use rs_merkle::{Hasher, MerkleProof, MerkleTree, algorithms::Keccak256};
use web3::types::Address;

use crate::{
    consts::NEW_LEAVES_CSV,
    csv_utils::{load_accounts, load_balances, load_keccak_merkle_leaves, load_tokens},
    types::{MerkleTreeLeaf, StorageAccount, StorageBalance},
};

pub fn run_create_keccak_leaves(
    accounts: String,
    balances: String,
    tokens: String,
    output: Option<String>,
) -> anyhow::Result<()> {
    println!("Creating new leaves from provided CSV files...");
    let stored_accounts = load_accounts(&accounts)
        .with_context(|| format!("Unable to process accounts file at {accounts}"))?;
    let stored_balances = load_balances(&balances)
        .with_context(|| format!("Unable to process balances file at {balances}"))?;
    let stored_tokens = load_tokens(&tokens)
        .with_context(|| format!("Unable to process tokens file at {tokens}"))?;
    let leaves = create_keccak_leaves(&stored_accounts, &stored_balances, &stored_tokens)?;
    let file_path = output.unwrap_or(NEW_LEAVES_CSV.to_string());
    save_leaves_to_csv(leaves, &file_path)?;
    println!("New leaves saved successfully to CSV {}", file_path);
    Ok(())
}

pub fn run_calculate_root_for_keccak_tree(leaves_path: Option<String>) -> anyhow::Result<()> {
    let leaves_file_path = leaves_path.unwrap_or(NEW_LEAVES_CSV.to_string());
    println!(
        "Calculating Keccak Merkle tree root from leaves file: {}",
        leaves_file_path
    );
    let leaves = load_keccak_merkle_leaves(&leaves_file_path)?;
    let root = calculate_root(&leaves).expect("No leaves found in the provided file");
    println!(
        "Calculated Keccak Merkle tree root: 0x{}",
        hex::encode(root)
    );
    Ok(())
}

pub fn run_create_proof_for_keccak_tree(
    account: Address,
    tokens: &[Address],
    leaves_path: Option<String>,
) -> anyhow::Result<()> {
    let leaves_file_path = leaves_path.unwrap_or(NEW_LEAVES_CSV.to_string());
    println!(
        "Creating Merkle proof for account {:?} and tokens {:?} from leaves file: {}",
        account, tokens, leaves_file_path
    );
    let leaves = load_keccak_merkle_leaves(&leaves_file_path)?;
    let proof = create_proof(&leaves, account, tokens)?;
    println!(
        "Merkle Proof for account {:?} and tokens {:?}: 0x{}",
        account,
        tokens,
        hex::encode(proof.to_bytes())
    );
    Ok(())
}

/// Calculates the Merkle root hash from leaves stored in a CSV file.
/// # Returns
/// Root hash if leaves were found, None otherwise
fn calculate_root(leaves: &HashMap<(Address, Address), MerkleTreeLeaf>) -> Option<[u8; 32]> {
    println!("Loaded {} leaves", leaves.len());
    let tree = create_tree(leaves);
    println!("Calculating Merkle root");
    tree.root()
}

/// Creates a Keccak Merkle tree from a collection of leaves.
fn create_tree(leaves: &HashMap<(Address, Address), MerkleTreeLeaf>) -> MerkleTree<Keccak256> {
    let leaf_hashes: Vec<[u8; 32]> = leaves
        .values()
        .par_bridge()
        .map(|leaf| {
            let leaf_bytes: Vec<u8> = leaf.clone().into();
            Keccak256::hash(&leaf_bytes)
        })
        .collect();
    MerkleTree::<Keccak256>::from_leaves(&leaf_hashes)
}

/// Creates a Merkle proof for a specific account and multiple tokens.
///
/// # Arguments
/// * `leaves` - Leaves mapped by (token_address, account_address)
/// * `account` - The account address to create a proof for
/// * `tokens` - The token addresses to create a proof for
fn create_proof(
    leaves: &HashMap<(Address, Address), MerkleTreeLeaf>,
    account: Address,
    tokens: &[Address],
) -> anyhow::Result<MerkleProof<Keccak256>> {
    let merkle_tree = create_tree(leaves);

    let leaves: anyhow::Result<Vec<_>> = tokens
        .iter()
        .copied()
        .map(|token_addresses| {
            leaves
                .get(&(token_addresses, account))
                .cloned()
                .context("Failed to find leaf index")
        })
        .collect();

    let indexes = find_leaf_indexes(&merkle_tree, leaves?)?;
    if indexes.len() != tokens.len() {
        anyhow::bail!("Some leaves were not found in the Merkle tree");
    }
    Ok(merkle_tree.proof(&indexes))
}

fn find_leaf_indexes(
    merkle_tree: &MerkleTree<Keccak256>,
    leaves: Vec<MerkleTreeLeaf>,
) -> anyhow::Result<Vec<usize>> {
    let leaf_hashes: Vec<[u8; 32]> = leaves
        .into_iter()
        .map(|leaf| {
            let leaf_bytes: Vec<u8> = leaf.into();
            Keccak256::hash(&leaf_bytes)
        })
        .collect();
    let mut indexes = vec![];
    for (position, tree_leaf) in merkle_tree
        .leaves()
        .context("Merkle tree is not initialized")?
        .iter()
        .enumerate()
    {
        if leaf_hashes.contains(tree_leaf) {
            indexes.push(position);
        }
    }

    Ok(indexes)
}

/// Creates Merkle tree leaves from account, balance, and token data.
/// Each leaf represents a (account, token, balance) combination.
/// # Returns
/// A vector of MerkleTreeLeaf objects ready for Keccak Merkle tree construction
fn create_keccak_leaves(
    accounts: &HashMap<u32, StorageAccount>,
    balances: &HashMap<u32, Vec<StorageBalance>>,
    tokens: &HashMap<u64, Address>,
) -> anyhow::Result<Vec<MerkleTreeLeaf>> {
    let mut leaves = vec![];
    for (account_id, account_balance) in balances.iter() {
        let account = accounts.get(account_id).context(format!(
            "Account {account_id} has balances, \
        but doesn't have a record in accounts.csv please check the consistency of the files"
        ))?;
        for balance in account_balance.iter() {
            let token_address = tokens
                .get(&(balance.coin_id as u64))
                .context(format!(
                    "Token with ID {} doesn't presented in the tokens file, please check the consistency of the files", balance.coin_id
                ))?;
            let leaf = MerkleTreeLeaf {
                account_address: Address::from_str(&account.address).expect("Correct address"),
                token_address: *token_address,
                balance: balance.balance.clone(),
            };
            leaves.push(leaf);
        }
    }
    Ok(leaves)
}

/// Saves Merkle tree leaves to a CSV file.
fn save_leaves_to_csv(leaves: Vec<MerkleTreeLeaf>, path: &str) -> anyhow::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut wtr = csv::Writer::from_writer(file);
    for leaf in leaves.into_iter() {
        wtr.serialize(leaf)?;
    }
    wtr.flush()?;
    Ok(())
}
