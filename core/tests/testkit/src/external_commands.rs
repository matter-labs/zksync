//! Run external commands from `$ZKSYNC_HOME/bin`
//!`$ZKSYNC_HOME/bin` should be in path.
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
    if deploy_script_out.starts_with("GOVERNANCE_ADDR=0x") {
        Some((
            String::from("GOVERNANCE_ADDR"),
            Address::from_str(&deploy_script_out["GOVERNANCE_ADDR=0x".len()..])
                .expect("can't parse contract address"),
        ))
    } else if deploy_script_out.starts_with("VERIFIER_ADDR=0x") {
        Some((
            String::from("VERIFIER_ADDR"),
            Address::from_str(&deploy_script_out["VERIFIER_ADDR=0x".len()..])
                .expect("can't parse contract address"),
        ))
    } else if deploy_script_out.starts_with("CONTRACT_ADDR=0x") {
        Some((
            String::from("CONTRACT_ADDR"),
            Address::from_str(&deploy_script_out["CONTRACT_ADDR=0x".len()..])
                .expect("can't parse contract address"),
        ))
    } else if deploy_script_out.starts_with("UPGRADE_GATEKEEPER_ADDR=0x") {
        Some((
            String::from("UPGRADE_GATEKEEPER_ADDR"),
            Address::from_str(&deploy_script_out["UPGRADE_GATEKEEPER_ADDR=0x".len()..])
                .expect("can't parse contract address"),
        ))
    } else if deploy_script_out.starts_with("TEST_ERC20=0x") {
        Some((
            String::from("TEST_ERC20"),
            Address::from_str(&deploy_script_out["TEST_ERC20=0x".len()..])
                .expect("can't parse contract address"),
        ))
    } else {
        None
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
    run_external_command(
        "revert-reason",
        &[&format!("0x{:x}", tx_hash), "http://localhost:7545"],
    )
}

pub fn deploy_contracts(use_prod_contracts: bool, genesis_root: Fr) -> Contracts {
    let mut args = Vec::new();
    args.push("--genesisRoot");
    let genesis_root = format!("0x{}", genesis_root.to_hex());
    args.push(genesis_root.as_str());
    // args.push(genesis_root)
    if use_prod_contracts {
        args.push("--prodContracts");
    }
    let stdout = run_external_command("deploy-testkit.sh", &args);

    let mut contracts = HashMap::new();
    for std_out_line in stdout.split_whitespace().collect::<Vec<_>>() {
        if let Some((name, address)) = get_contract_address(std_out_line) {
            contracts.insert(name, address);
        }
    }

    Contracts {
        governance: contracts
            .remove("GOVERNANCE_ADDR")
            .expect("GOVERNANCE_ADDR missing"),
        verifier: contracts
            .remove("VERIFIER_ADDR")
            .expect("VERIFIER_ADDR missing"),
        contract: contracts
            .remove("CONTRACT_ADDR")
            .expect("CONTRACT_ADDR missing"),
        upgrade_gatekeeper: contracts
            .remove("UPGRADE_GATEKEEPER_ADDR")
            .expect("UPGRADE_GATEKEEPER_ADDR missing"),
        test_erc20_address: contracts.remove("TEST_ERC20").expect("TEST_ERC20 missing"),
    }
}

pub fn run_upgrade_franklin(franklin_address: Address, upgrade_gatekeeper_address: Address) {
    run_external_command(
        "test-upgrade-franklin.sh",
        &[
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
    let stdout = run_external_command("print-test-accounts.sh", &[]);

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
