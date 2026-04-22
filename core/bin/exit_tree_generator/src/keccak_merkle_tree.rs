use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
};

use anyhow::{Context, ensure};
use bigdecimal::BigDecimal;
use ethabi::ethereum_types::U256;
use num_bigint::ToBigInt;
use rayon::prelude::*;
use rs_merkle::{Hasher, MerkleTree};
use serde::Serialize;
use web3::{signing::keccak256, types::Address};

use crate::{
    consts::NEW_LEAVES_CSV,
    csv_utils::{load_accounts, load_balances, load_keccak_merkle_leaves, load_tokens},
    types::{MerkleTreeLeaf, StorageAccount, StorageBalance},
};

/// OpenZeppelin-compatible Keccak-256 hasher: sorts each pair before hashing
/// and propagates a lone node unchanged, matching `MerkleProof.verifyCalldata` on L1.
#[derive(Clone)]
struct OzKeccak256;

impl Hasher for OzKeccak256 {
    type Hash = [u8; 32];

    fn hash(data: &[u8]) -> [u8; 32] {
        keccak256(data)
    }

    fn concat_and_hash(left: &[u8; 32], right: Option<&[u8; 32]>) -> [u8; 32] {
        let Some(right) = right else { return *left };
        let (a, b) = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        let mut buf = [0u8; 64];
        buf[..32].copy_from_slice(a);
        buf[32..].copy_from_slice(b);
        Self::hash(&buf)
    }
}

#[derive(Debug, serde::Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MerkleProofOutput {
    pub claim_index: u64,
    pub account_address: Address,
    pub token_address: Address,
    pub balance: String,
    pub merkle_root: String,
    pub merkle_path: Vec<String>,
    pub leaf: String,
}

pub fn run_create_keccak_leaves(
    accounts_path: &str,
    balances_path: &str,
    tokens_path: &str,
    output: Option<String>,
) -> anyhow::Result<()> {
    println!("Creating new leaves from provided CSV files...");
    let stored_accounts = load_accounts(accounts_path)
        .with_context(|| format!("Unable to process accounts file at {accounts_path}"))?;
    let stored_balances = load_balances(balances_path)
        .with_context(|| format!("Unable to process balances file at {balances_path}"))?;
    let stored_tokens = load_tokens(tokens_path)
        .with_context(|| format!("Unable to process tokens file at {tokens_path}"))?;
    let leaves = create_keccak_leaves(&stored_accounts, &stored_balances, &stored_tokens)?;

    let file_path = output.unwrap_or(NEW_LEAVES_CSV.to_string());
    save_leaves_to_csv(&leaves, &file_path)?;
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
    let root = calculate_root_checked(&leaves)?.expect("No leaves found in the provided file");
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
    let proofs = create_proof(&leaves, account, tokens)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&proofs).context("failed to serialize proof output")?
    );
    Ok(())
}

/// Calculates the L1-compatible Merkle root for the given leaves.
/// Returns `None` when there are no leaves.
pub fn calculate_root_checked(leaves: &[MerkleTreeLeaf]) -> anyhow::Result<Option<[u8; 32]>> {
    println!("Loaded {} leaves", leaves.len());
    Ok(build_tree(leaves)?.root())
}

/// Calculates the L1-compatible Merkle root together with a proof for every leaf,
/// returned in ascending `claim_index` order. Returns `(None, empty)` when there are no leaves.
pub fn calculate_root_and_proofs_checked(
    leaves: &[MerkleTreeLeaf],
) -> anyhow::Result<(Option<[u8; 32]>, Vec<Vec<[u8; 32]>>)> {
    let ordered = sort_and_validate_leaves(leaves)?;
    if ordered.is_empty() {
        return Ok((None, Vec::new()));
    }

    let tree = tree_from_ordered(&ordered);
    let root = tree.root();
    let proofs = ordered
        .iter()
        .map(|leaf| {
            tree.proof(&[leaf.claim_index as usize])
                .proof_hashes()
                .to_vec()
        })
        .collect();

    Ok((root, proofs))
}

/// Creates Merkle proofs for a specific account and a list of tokens.
pub fn create_proof(
    leaves: &[MerkleTreeLeaf],
    account: Address,
    tokens: &[Address],
) -> anyhow::Result<Vec<MerkleProofOutput>> {
    let ordered = sort_and_validate_leaves(leaves)?;
    let tree = tree_from_ordered(&ordered);
    let root = tree
        .root()
        .context("No leaves found in the provided file")?;

    let lookup: BTreeMap<(Address, Address), &MerkleTreeLeaf> = ordered
        .iter()
        .map(|leaf| ((leaf.account_address, leaf.token_address), leaf))
        .collect();

    tokens
        .iter()
        .copied()
        .map(|token_address| {
            let leaf = lookup
                .get(&(account, token_address))
                .copied()
                .with_context(|| {
                    format!(
                        "Failed to find leaf for account {:?} and token {:?}",
                        account, token_address
                    )
                })?;
            let proof = tree.proof(&[leaf.claim_index as usize]);

            Ok(MerkleProofOutput {
                claim_index: leaf.claim_index,
                account_address: leaf.account_address,
                token_address: leaf.token_address,
                balance: leaf.balance.clone(),
                merkle_root: format!("0x{}", hex::encode(root)),
                merkle_path: proof
                    .proof_hashes()
                    .iter()
                    .map(|hash| format!("0x{}", hex::encode(hash)))
                    .collect(),
                leaf: format!("0x{}", hex::encode(leaf_hash(leaf))),
            })
        })
        .collect()
}

/// Verifies a proof the same way `MerkleProofUpgradeable.verifyCalldata` does on L1:
/// fold the path with sorted-pair Keccak hashing and compare against the root.
pub fn verify_proof(leaf: [u8; 32], proof_hashes: &[[u8; 32]], expected_root: [u8; 32]) -> bool {
    let computed = proof_hashes.iter().fold(leaf, |acc, sibling| {
        OzKeccak256::concat_and_hash(&acc, Some(sibling))
    });
    computed == expected_root
}

/// Packed Keccak-256 hash of a leaf, matching the on-chain computation
/// `keccak256(abi.encodePacked(index, claimant, token, amount))`.
pub fn leaf_hash(leaf: &MerkleTreeLeaf) -> [u8; 32] {
    let mut encoded = Vec::with_capacity(32 + 20 + 20 + 32);
    encoded.extend_from_slice(&u256_to_bytes32(U256::from(leaf.claim_index)));
    encoded.extend_from_slice(leaf.account_address.as_bytes());
    encoded.extend_from_slice(leaf.token_address.as_bytes());
    encoded.extend_from_slice(&u256_to_bytes32(leaf.balance_as_u256()));
    OzKeccak256::hash(&encoded)
}

fn build_tree(leaves: &[MerkleTreeLeaf]) -> anyhow::Result<MerkleTree<OzKeccak256>> {
    let ordered = sort_and_validate_leaves(leaves)?;
    Ok(tree_from_ordered(&ordered))
}

fn tree_from_ordered(ordered: &[MerkleTreeLeaf]) -> MerkleTree<OzKeccak256> {
    let leaf_hashes: Vec<[u8; 32]> = ordered.par_iter().map(leaf_hash).collect();
    MerkleTree::<OzKeccak256>::from_leaves(&leaf_hashes)
}

fn u256_to_bytes32(value: U256) -> [u8; 32] {
    let mut out = [0u8; 32];
    value.to_big_endian(&mut out);
    out
}

/// Creates Merkle tree leaves from account, balance, and token data.
/// Leaves are canonically ordered by `(account_address, token_address)` and indexed from zero.
fn create_keccak_leaves(
    accounts: &HashMap<u32, StorageAccount>,
    balances: &HashMap<u32, Vec<StorageBalance>>,
    tokens: &HashMap<u64, Address>,
) -> anyhow::Result<Vec<MerkleTreeLeaf>> {
    let mut aggregate = BTreeMap::<(Address, Address), U256>::new();

    for (account_id, account_balance) in balances.iter() {
        let account = accounts.get(account_id).context(format!(
            "Account {account_id} has balances, \
        but doesn't have a record in accounts.csv please check the consistency of the files"
        ))?;
        let account_address = Address::from_str(&account.address).expect("Correct address");

        for balance in account_balance.iter() {
            let token_address = tokens
                .get(&(balance.coin_id as u64))
                .context(format!(
                    "Token with ID {} doesn't presented in the tokens file, please check the consistency of the files", balance.coin_id
                ))?;
            let amount = parse_decimal_u256(&balance.balance)
                .with_context(|| format!("Invalid balance amount: {}", balance.balance))?;

            let slot = aggregate
                .entry((account_address, *token_address))
                .or_insert_with(U256::zero);
            let (sum, overflow) = slot.overflowing_add(amount);
            ensure!(!overflow, "Balance overflow for account {account_id}");
            *slot = sum;
        }
    }

    Ok(aggregate
        .into_iter()
        .enumerate()
        .map(
            |(claim_index, ((account_address, token_address), balance))| MerkleTreeLeaf {
                claim_index: claim_index as u64,
                account_address,
                token_address,
                balance: balance.to_string(),
            },
        )
        .collect())
}

fn sort_and_validate_leaves(leaves: &[MerkleTreeLeaf]) -> anyhow::Result<Vec<MerkleTreeLeaf>> {
    let mut ordered = leaves.to_vec();
    ordered.sort_by_key(|leaf| leaf.claim_index);

    for (expected_index, leaf) in ordered.iter().enumerate() {
        ensure!(
            leaf.claim_index == expected_index as u64,
            "Leaves must have contiguous claim_index values starting from zero"
        );
    }

    Ok(ordered)
}

fn parse_decimal_u256(value: &str) -> anyhow::Result<U256> {
    Ok(U256::from_big_endian(
        BigDecimal::from_str(value)
            .context("invalid decimal balance")?
            .to_bigint()
            .context("balance is not an integer")?
            .to_bytes_be()
            .1
            .as_slice(),
    ))
}

/// Saves Merkle tree leaves to a CSV file.
fn save_leaves_to_csv<T: Serialize>(leaves: &[T], path: &str) -> anyhow::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut wtr = csv::Writer::from_writer(file);
    for leaf in leaves.iter() {
        wtr.serialize(leaf)?;
    }
    wtr.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::types::{StorageAccount, StorageBalance};

    fn sample_leaves() -> Vec<MerkleTreeLeaf> {
        vec![
            MerkleTreeLeaf {
                claim_index: 0,
                account_address: Address::from_low_u64_be(1),
                token_address: Address::from_low_u64_be(11),
                balance: "10".to_string(),
            },
            MerkleTreeLeaf {
                claim_index: 1,
                account_address: Address::from_low_u64_be(1),
                token_address: Address::from_low_u64_be(12),
                balance: "20".to_string(),
            },
            MerkleTreeLeaf {
                claim_index: 2,
                account_address: Address::from_low_u64_be(2),
                token_address: Address::from_low_u64_be(11),
                balance: "30".to_string(),
            },
        ]
    }

    #[test]
    fn create_keccak_leaves_assigns_canonical_indexes() {
        let accounts = HashMap::from([
            (
                2u32,
                StorageAccount {
                    id: 2,
                    nonce: 0,
                    address: format!("{:#x}", Address::from_low_u64_be(2)),
                    pubkey_hash: "sync:00".to_string(),
                },
            ),
            (
                1u32,
                StorageAccount {
                    id: 1,
                    nonce: 0,
                    address: format!("{:#x}", Address::from_low_u64_be(1)),
                    pubkey_hash: "sync:00".to_string(),
                },
            ),
        ]);
        let balances = HashMap::from([
            (
                1u32,
                vec![
                    StorageBalance {
                        account_id: 1,
                        coin_id: 2,
                        balance: "20".to_string(),
                    },
                    StorageBalance {
                        account_id: 1,
                        coin_id: 1,
                        balance: "10".to_string(),
                    },
                ],
            ),
            (
                2u32,
                vec![StorageBalance {
                    account_id: 2,
                    coin_id: 1,
                    balance: "30".to_string(),
                }],
            ),
        ]);
        let tokens = HashMap::from([
            (1u64, Address::from_low_u64_be(11)),
            (2u64, Address::from_low_u64_be(12)),
        ]);

        let leaves = create_keccak_leaves(&accounts, &balances, &tokens).unwrap();

        assert_eq!(leaves.len(), 3);
        assert_eq!(leaves[0].claim_index, 0);
        assert_eq!(leaves[0].account_address, Address::from_low_u64_be(1));
        assert_eq!(leaves[0].token_address, Address::from_low_u64_be(11));
        assert_eq!(leaves[1].claim_index, 1);
        assert_eq!(leaves[1].account_address, Address::from_low_u64_be(1));
        assert_eq!(leaves[1].token_address, Address::from_low_u64_be(12));
        assert_eq!(leaves[2].claim_index, 2);
        assert_eq!(leaves[2].account_address, Address::from_low_u64_be(2));
        assert_eq!(leaves[2].token_address, Address::from_low_u64_be(11));
    }

    #[test]
    fn oz_hasher_sorts_pairs_and_propagates_lone_leaf() {
        let a = [0xff; 32];
        let mut b = [0u8; 32];
        b[31] = 1;
        assert_eq!(
            OzKeccak256::concat_and_hash(&a, Some(&b)),
            OzKeccak256::concat_and_hash(&b, Some(&a))
        );
        assert_eq!(OzKeccak256::concat_and_hash(&a, None), a);
    }

    #[test]
    fn create_proof_round_trips_against_root() {
        let leaves = sample_leaves();
        let proofs = create_proof(
            &leaves,
            Address::from_low_u64_be(1),
            &[Address::from_low_u64_be(12)],
        )
        .unwrap();
        let proof = &proofs[0];
        let root = parse_hash_hex(&proof.merkle_root);
        let leaf = parse_hash_hex(&proof.leaf);
        let path = proof
            .merkle_path
            .iter()
            .map(|item| parse_hash_hex(item))
            .collect::<Vec<_>>();

        assert!(verify_proof(leaf, &path, root));
    }

    #[test]
    fn proofs_verify_for_every_leaf_with_odd_leaf_count() {
        let leaves = sample_leaves();
        let root = calculate_root_checked(&leaves).unwrap().unwrap();
        for leaf in &leaves {
            let proofs =
                create_proof(&leaves, leaf.account_address, &[leaf.token_address]).unwrap();
            let proof = &proofs[0];
            let path = proof
                .merkle_path
                .iter()
                .map(|item| parse_hash_hex(item))
                .collect::<Vec<_>>();
            assert!(verify_proof(leaf_hash(leaf), &path, root));
        }
    }

    #[test]
    fn calculate_root_and_proofs_matches_per_leaf_proof() {
        let leaves = sample_leaves();
        let (root, proofs) = calculate_root_and_proofs_checked(&leaves).unwrap();
        let root = root.unwrap();

        assert_eq!(proofs.len(), leaves.len());
        for (leaf, proof) in leaves.iter().zip(proofs.iter()) {
            assert!(verify_proof(leaf_hash(leaf), proof, root));
        }
    }

    #[test]
    fn calculate_root_and_proofs_returns_none_for_empty_input() {
        let (root, proofs) = calculate_root_and_proofs_checked(&[]).unwrap();
        assert!(root.is_none());
        assert!(proofs.is_empty());
    }

    #[test]
    fn single_leaf_tree_has_leaf_root_and_empty_proof() {
        let leaf = MerkleTreeLeaf {
            claim_index: 0,
            account_address: Address::from_low_u64_be(1),
            token_address: Address::from_low_u64_be(11),
            balance: "10".to_string(),
        };
        let leaves = vec![leaf.clone()];

        let root = calculate_root_checked(&leaves).unwrap().unwrap();
        assert_eq!(root, leaf_hash(&leaf));

        let proofs = create_proof(&leaves, leaf.account_address, &[leaf.token_address]).unwrap();
        assert!(proofs[0].merkle_path.is_empty());
        assert!(verify_proof(leaf_hash(&leaf), &[], root));
    }

    fn parse_hash_hex(value: &str) -> [u8; 32] {
        let bytes = hex::decode(value.trim_start_matches("0x")).unwrap();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&bytes);
        hash
    }
}
