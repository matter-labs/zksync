use std::{
    env, fs,
    path::{Path, PathBuf},
    str::FromStr,
    time::UNIX_EPOCH,
};

use alloy::{
    node_bindings::Anvil,
    primitives::Address,
    providers::{Provider, ProviderBuilder},
    sol,
};
use anyhow::{Context, Result, ensure};
use dotenvy::from_path_override;
use serde::{Deserialize, Serialize};
use web3::signing::keccak256;
use zksync_exit_tree_generator::{
    csv_utils::load_keccak_merkle_leaves,
    keccak_merkle_tree::{
        MerkleProofOutput, calculate_root_checked, create_proof, run_create_keccak_leaves,
        verify_proof,
    },
};

const SNAPSHOT_TREE_CACHE_VERSION: &str = "snapshot-tree-e2e-v4";

sol! {
    #[sol(rpc)]
    interface IGovernance {
        function tokenIds(address token) external view returns (uint16);
        function tokenAddresses(uint16 tokenId) external view returns (address);
    }
}

#[derive(Debug, Deserialize)]
struct MainnetExitToolConfig {
    governance_addr: String,
    contract_addr: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotTreeCacheMetadata {
    cache_version: String,
    leaf_count: usize,
    root: String,
    sample_proof: MerkleProofOutput,
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires local CSV fixtures, a mainnet RPC URL, and an anvil binary"]
async fn snapshot_tree_e2e_builds_root_and_verifies_proof() -> Result<()> {
    let dotenv_path = manifest_dir().join(".env");
    if dotenv_path.exists() {
        from_path_override(&dotenv_path)
            .with_context(|| format!("failed to load {}", dotenv_path.display()))?;
    }

    let data_dir = manifest_dir().join("data");
    ensure!(
        data_dir.exists(),
        "missing local fixture directory: {}",
        data_dir.display()
    );

    let cache = load_or_build_snapshot_tree_cache(&data_dir)?;
    ensure!(cache.leaf_count > 0, "expected non-empty claim tree");

    let root = parse_hash_hex(&cache.root)?;
    ensure!(root != [0u8; 32], "unexpected zero Merkle root");

    verify_claim_proof(&cache.sample_proof, root)?;

    let config = load_mainnet_exit_tool_config()?;
    let governance_address = Address::from_str(&config.governance_addr)?;
    let zksync_address = Address::from_str(&config.contract_addr)?;

    let rpc_url = mainnet_rpc_url()?;
    let anvil = Anvil::new().fork(rpc_url).spawn();
    let provider = ProviderBuilder::new().connect_http(anvil.endpoint().parse()?);

    let zksync_code = provider.get_code_at(zksync_address).await?;
    ensure!(
        !zksync_code.is_empty(),
        "expected zkSync contract code at {} on the fork",
        zksync_address
    );

    let dai = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f")?;
    let usdc = Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")?;
    let governance = IGovernance::new(governance_address, provider.clone());
    let dai_token_id = governance.tokenIds(dai).call().await?;
    let usdc_token_id = governance.tokenIds(usdc).call().await?;
    let dai_address = governance.tokenAddresses(1).call().await?;
    let usdc_address = governance.tokenAddresses(2).call().await?;

    assert_eq!(
        dai_token_id, 1,
        "DAI token id mismatch on forked mainnet governance"
    );
    assert_eq!(
        usdc_token_id, 2,
        "USDC token id mismatch on forked mainnet governance"
    );
    assert_eq!(dai_address, dai, "DAI address mismatch for token id 1");
    assert_eq!(usdc_address, usdc, "USDC address mismatch for token id 2");

    Ok(())
}

fn load_or_build_snapshot_tree_cache(data_dir: &Path) -> Result<SnapshotTreeCacheMetadata> {
    let accounts_path = data_dir.join("accounts.csv");
    let balances_path = data_dir.join("balances.csv");
    let tokens_path = data_dir.join("tokens.csv");
    let cache_key = snapshot_tree_cache_key(&accounts_path, &balances_path, &tokens_path)?;
    let cache_dir = data_dir.join("cache").join(cache_key);
    let metadata_path = cache_dir.join("metadata.json");

    if metadata_path.exists() {
        let metadata = fs::read_to_string(&metadata_path)
            .with_context(|| format!("failed to read {}", metadata_path.display()))?;
        return serde_json::from_str(&metadata)
            .with_context(|| format!("failed to parse {}", metadata_path.display()));
    }

    fs::create_dir_all(&cache_dir)
        .with_context(|| format!("failed to create {}", cache_dir.display()))?;
    let leaves_path = cache_dir.join("new_leaves.csv");
    run_create_keccak_leaves(
        path_to_str(&accounts_path)?,
        path_to_str(&balances_path)?,
        path_to_str(&tokens_path)?,
        Some(path_to_str(&leaves_path)?.to_string()),
    )?;
    let leaves = load_keccak_merkle_leaves(path_to_str(&leaves_path)?)?;
    ensure!(!leaves.is_empty(), "expected non-empty claim tree");

    let root = calculate_root_checked(&leaves)?
        .context("expected the real CSV fixture set to produce a Merkle root")?;

    let sample_leaf = leaves
        .first()
        .context("expected at least one leaf for proof generation")?;

    let metadata = SnapshotTreeCacheMetadata {
        cache_version: SNAPSHOT_TREE_CACHE_VERSION.to_string(),
        leaf_count: leaves.len(),
        root: format!("0x{}", hex::encode(root)),
        sample_proof: create_proof(
            &leaves,
            sample_leaf.account_address,
            &[sample_leaf.token_address],
        )?
        .into_iter()
        .next()
        .context("expected proof output for the sample leaf")?,
    };

    fs::write(&metadata_path, serde_json::to_vec_pretty(&metadata)?)
        .with_context(|| format!("failed to write {}", metadata_path.display()))?;

    Ok(metadata)
}

fn snapshot_tree_cache_key(
    accounts_path: &Path,
    balances_path: &Path,
    tokens_path: &Path,
) -> Result<String> {
    let mut key_material = Vec::new();
    key_material.extend_from_slice(SNAPSHOT_TREE_CACHE_VERSION.as_bytes());
    append_cache_file_fingerprint(&mut key_material, accounts_path)?;
    append_cache_file_fingerprint(&mut key_material, balances_path)?;
    append_cache_file_fingerprint(&mut key_material, tokens_path)?;
    Ok(hex::encode(keccak256(&key_material)))
}

fn append_cache_file_fingerprint(out: &mut Vec<u8>, path: &Path) -> Result<()> {
    let metadata =
        fs::metadata(path).with_context(|| format!("failed to stat {}", path.display()))?;
    let modified = metadata
        .modified()
        .with_context(|| format!("failed to read modified time for {}", path.display()))?
        .duration_since(UNIX_EPOCH)
        .with_context(|| format!("system time before UNIX_EPOCH for {}", path.display()))?;

    out.extend_from_slice(path_to_str(path)?.as_bytes());
    out.extend_from_slice(&metadata.len().to_le_bytes());
    out.extend_from_slice(&modified.as_secs().to_le_bytes());
    out.extend_from_slice(&modified.subsec_nanos().to_le_bytes());
    Ok(())
}

fn verify_claim_proof(proof: &MerkleProofOutput, expected_root: [u8; 32]) -> Result<()> {
    let proof_root = parse_hash_hex(&proof.merkle_root)?;
    let proof_leaf = parse_hash_hex(&proof.leaf)?;
    let proof_path = proof
        .merkle_path
        .iter()
        .map(|item| parse_hash_hex(item))
        .collect::<Result<Vec<_>>>()?;

    ensure!(
        proof_root == expected_root,
        "proof root did not match the computed root"
    );
    ensure!(
        verify_proof(proof_leaf, &proof_path, expected_root),
        "generated proof did not verify against the computed root"
    );
    Ok(())
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn path_to_str(path: &Path) -> Result<&str> {
    path.to_str()
        .with_context(|| format!("path is not valid UTF-8: {}", path.display()))
}

fn mainnet_rpc_url() -> Result<String> {
    env::var("EXIT_TREE_MAINNET_RPC_URL")
        .or_else(|_| env::var("MAINNET_RPC_URL"))
        .or_else(|_| env::var("ETH_CLIENT_WEB3_URL"))
        .context("set EXIT_TREE_MAINNET_RPC_URL, MAINNET_RPC_URL, or ETH_CLIENT_WEB3_URL")
}

fn load_mainnet_exit_tool_config() -> Result<MainnetExitToolConfig> {
    let config_path = manifest_dir()
        .join("../../../docker/exit-tool/configs/mainnet.json")
        .canonicalize()
        .context("failed to resolve mainnet exit-tool config path")?;
    let config = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    serde_json::from_str(&config)
        .with_context(|| format!("failed to parse {}", config_path.display()))
}

fn parse_hash_hex(value: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(value.trim_start_matches("0x"))
        .with_context(|| format!("failed to decode hash hex: {value}"))?;
    ensure!(
        bytes.len() == 32,
        "expected 32-byte hash, got {}",
        bytes.len()
    );
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&bytes);
    Ok(hash)
}
