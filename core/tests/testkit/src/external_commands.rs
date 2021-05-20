//! Run external commands from the zk toolkit
//! `zk` script should be in path.
//!
use std::collections::HashMap;
use std::process::Command;
use std::str::FromStr;
use web3::types::{Address, H256};

use serde::{Deserialize, Serialize};
use zksync_crypto::convert::FeConvert;
use zksync_crypto::Fr;

#[derive(Debug, Clone)]
pub struct Contracts {
    pub governance: Address,
    pub verifier: Address,
    pub contract: Address,
    pub upgrade_gatekeeper: Address,
    pub test_erc20_address: Address,
}

fn get_contract_address(deploy_script_out: &str) -> Option<(String, Address)> {
    if let Some(output) = deploy_script_out.strip_prefix("CONTRACTS_GOVERNANCE_ADDR=0x") {
        Some((
            String::from("CONTRACTS_GOVERNANCE_ADDR"),
            Address::from_str(output).expect("can't parse contract address"),
        ))
    } else if let Some(output) = deploy_script_out.strip_prefix("CONTRACTS_VERIFIER_ADDR=0x") {
        Some((
            String::from("CONTRACTS_VERIFIER_ADDR"),
            Address::from_str(output).expect("can't parse contract address"),
        ))
    } else if let Some(output) = deploy_script_out.strip_prefix("CONTRACTS_CONTRACT_ADDR=0x") {
        Some((
            String::from("CONTRACTS_CONTRACT_ADDR"),
            Address::from_str(output).expect("can't parse contract address"),
        ))
    } else if let Some(output) =
        deploy_script_out.strip_prefix("CONTRACTS_UPGRADE_GATEKEEPER_ADDR=0x")
    {
        Some((
            String::from("CONTRACTS_UPGRADE_GATEKEEPER_ADDR"),
            Address::from_str(output).expect("can't parse contract address"),
        ))
    } else {
        deploy_script_out
            .strip_prefix("CONTRACTS_TEST_ERC20=0x")
            .map(|output| {
                (
                    String::from("CONTRACTS_TEST_ERC20"),
                    Address::from_str(output).expect("can't parse contract address"),
                )
            })
    }
}

/// Runs external command and returns stdout output
fn run_external_command(command: &str, args: &[&str]) -> String {
    let result = Command::new(command)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to execute command: {}, err: {}", command, e));

    let stdout = String::from_utf8(result.stdout).expect("stdout is not valid utf8");
    let stderr = String::from_utf8(result.stderr).expect("stderr is not valid utf8");

    if !result.status.success() {
        panic!(
            "failed to run exetrnal command {}:\nstdout: {}\nstderr: {}",
            command, stdout, stderr
        );
    }
    stdout
}

pub fn js_revert_reason(tx_hash: &H256) -> String {
    let web3_urls =
        std::env::var("ETH_CLIENT_WEB3_URL").expect("ETH_CLIENT_WEB3_URL should be installed");
    let web3_urls: Vec<&str> = web3_urls.split(',').collect();
    run_external_command(
        "zk",
        &[
            "run",
            "revert-reason",
            &format!("0x{:x}", tx_hash),
            web3_urls.first().expect("At least one should exist"),
        ],
    )
}

pub fn deploy_contracts(use_prod_contracts: bool, genesis_root: Fr) -> Contracts {
    let mut args = vec!["run", "deploy-testkit", "--genesisRoot"];
    let genesis_root = format!("0x{}", genesis_root.to_hex());
    args.push(genesis_root.as_str());
    if use_prod_contracts {
        args.push("--prodContracts");
    }
    let stdout = run_external_command("zk", &args);

    let mut contracts = HashMap::new();
    for std_out_line in stdout.split_whitespace().collect::<Vec<_>>() {
        if let Some((name, address)) = get_contract_address(std_out_line) {
            contracts.insert(name, address);
        }
    }

    Contracts {
        governance: contracts
            .remove("CONTRACTS_GOVERNANCE_ADDR")
            .expect("GOVERNANCE_ADDR missing"),
        verifier: contracts
            .remove("CONTRACTS_VERIFIER_ADDR")
            .expect("VERIFIER_ADDR missing"),
        contract: contracts
            .remove("CONTRACTS_CONTRACT_ADDR")
            .expect("CONTRACT_ADDR missing"),
        upgrade_gatekeeper: contracts
            .remove("CONTRACTS_UPGRADE_GATEKEEPER_ADDR")
            .expect("UPGRADE_GATEKEEPER_ADDR missing"),
        test_erc20_address: contracts
            .remove("CONTRACTS_TEST_ERC20")
            .expect("TEST_ERC20 missing"),
    }
}

pub fn run_upgrade_franklin(franklin_address: Address, upgrade_gatekeeper_address: Address) {
    run_external_command(
        "zk",
        &[
            "run",
            "test-upgrade",
            &format!("0x{:x}", franklin_address),
            &format!("0x{:x}", upgrade_gatekeeper_address),
        ],
    );
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ETHAccountInfo {
    pub address: Address,
    pub private_key: H256,
}

/// First is vec of test acccounts, second is commit account
pub fn get_test_accounts() -> (Vec<ETHAccountInfo>, ETHAccountInfo) {
    let stdout = run_external_command("zk", &["run", "test-accounts"]);

    if let Ok(mut parsed) = serde_json::from_str::<Vec<ETHAccountInfo>>(&stdout) {
        let commit_account = parsed.remove(0);
        assert!(
            !parsed.is_empty(),
            "can't use testkit without test accounts"
        );
        return (parsed, commit_account);
    }

    panic!("Print test accounts script output is not parsed correctly")
}
