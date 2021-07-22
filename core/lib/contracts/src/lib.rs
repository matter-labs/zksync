use ethabi::Contract;
use std::fs;
use std::io;
use std::str::FromStr;

const ZKSYNC_CONTRACT_FILE_V0: &str = "contracts/old_contracts/ZkSync.json";
const ZKSYNC_CONTRACT_FILE_V1: &str = "contracts/old_contracts/ZkSync.json";
const ZKSYNC_CONTRACT_FILE_V2: &str = "contracts/old_contracts/ZkSync.json";
const ZKSYNC_CONTRACT_FILE_V3: &str = "contracts/old_contracts/ZkSync.json";
const ZKSYNC_CONTRACT_FILE_V4: &str =
    "contracts/artifacts/cache/solpp-generated-contracts/ZkSync.sol/ZkSync.json";
const GOVERNANCE_CONTRACT_FILE: &str =
    "contracts/artifacts/cache/solpp-generated-contracts/Governance.sol/Governance.json";
const IERC20_CONTRACT_FILE: &str =
    "contracts/artifacts/cache/solpp-generated-contracts/IERC20.sol/IERC20.json";
const IEIP1271_CONTRACT_FILE: &str =
    "contracts/artifacts/cache/solpp-generated-contracts/dev-contracts/IEIP1271.sol/IEIP1271.json";
const UPGRADE_GATEKEEPER_CONTRACT_FILE: &str =
    "contracts/artifacts/cache/solpp-generated-contracts/UpgradeGatekeeper.sol/UpgradeGatekeeper.json";
const FORCED_EXIT_CONTRACT_FILE: &str =
    "contracts/artifacts/cache/solpp-generated-contracts/ForcedExit.sol/ForcedExit.json";

fn read_file_to_json_value(path: &str) -> io::Result<serde_json::Value> {
    let zksync_home = std::env::var("ZKSYNC_HOME").unwrap_or_else(|_| ".".into());
    let path = std::path::Path::new(&zksync_home).join(path);
    let contents = fs::read_to_string(path)?;
    let val = serde_json::Value::from_str(&contents)?;
    Ok(val)
}

pub fn zksync_contract_v0() -> Contract {
    let abi_string = read_file_to_json_value(ZKSYNC_CONTRACT_FILE_V0)
        .expect("couldn't read ZKSYNC_CONTRACT_FILE_V0")
        .get("abi")
        .expect("couldn't get abi from ZKSYNC_CONTRACT_FILE_V0")
        .to_string();
    Contract::load(abi_string.as_bytes()).expect("zksync contract abi")
}

pub fn zksync_contract_v1() -> Contract {
    let abi_string = read_file_to_json_value(ZKSYNC_CONTRACT_FILE_V1)
        .expect("couldn't read ZKSYNC_CONTRACT_FILE_V1")
        .get("abi")
        .expect("couldn't get abi from ZKSYNC_CONTRACT_FILE_V1")
        .to_string();
    Contract::load(abi_string.as_bytes()).expect("zksync contract abi")
}

pub fn zksync_contract_v2() -> Contract {
    let abi_string = read_file_to_json_value(ZKSYNC_CONTRACT_FILE_V2)
        .expect("couldn't read ZKSYNC_CONTRACT_FILE_V2")
        .get("abi")
        .expect("couldn't get abi from ZKSYNC_CONTRACT_FILE_V2")
        .to_string();
    Contract::load(abi_string.as_bytes()).expect("zksync contract abi")
}

pub fn zksync_contract_v3() -> Contract {
    let abi_string = read_file_to_json_value(ZKSYNC_CONTRACT_FILE_V3)
        .expect("couldn't read ZKSYNC_CONTRACT_FILE_V3")
        .get("abi")
        .expect("couldn't get abi from ZKSYNC_CONTRACT_FILE_V3")
        .to_string();
    Contract::load(abi_string.as_bytes()).expect("zksync contract abi")
}

pub fn zksync_contract() -> Contract {
    let abi_string = read_file_to_json_value(ZKSYNC_CONTRACT_FILE_V4)
        .expect("couldn't read ZKSYNC_CONTRACT_FILE_V4")
        .get("abi")
        .expect("couldn't get abi from ZKSYNC_CONTRACT_FILE_V4")
        .to_string();
    Contract::load(abi_string.as_bytes()).expect("zksync contract abi")
}

pub fn governance_contract() -> Contract {
    let abi_string = read_file_to_json_value(GOVERNANCE_CONTRACT_FILE)
        .expect("couldn't read GOVERNANCE_CONTRACT_FILE")
        .get("abi")
        .expect("couldn't get abi from GOVERNANCE_CONTRACT_FILE")
        .to_string();
    Contract::load(abi_string.as_bytes()).expect("governance contract abi")
}

pub fn erc20_contract() -> Contract {
    let abi_string = read_file_to_json_value(IERC20_CONTRACT_FILE)
        .expect("couldn't read IERC20_CONTRACT_FILE")
        .get("abi")
        .expect("couldn't get abi from IERC20_CONTRACT_FILE")
        .to_string();
    Contract::load(abi_string.as_bytes()).expect("erc20 contract abi")
}

pub fn eip1271_contract() -> Contract {
    let abi_string = read_file_to_json_value(IEIP1271_CONTRACT_FILE)
        .expect("couldn't read IEIP1271_CONTRACT_FILE")
        .get("abi")
        .expect("couldn't get abi from IEIP1271_CONTRACT_FILE")
        .to_string();
    Contract::load(abi_string.as_bytes()).expect("erc20 contract abi")
}
pub fn upgrade_gatekeeper() -> Contract {
    let abi_string = read_file_to_json_value(UPGRADE_GATEKEEPER_CONTRACT_FILE)
        .expect("couldn't read UPGRADE_GATEKEEPER_CONTRACT_FILE")
        .get("abi")
        .expect("couldn't get abi from UPGRADE_GATEKEEPER_CONTRACT_FILE")
        .to_string();
    Contract::load(abi_string.as_bytes()).expect("gatekeeper contract abi")
}

pub fn forced_exit_contract() -> Contract {
    let abi_string = read_file_to_json_value(FORCED_EXIT_CONTRACT_FILE)
        .expect("couldn't read FORCED_EXIT_CONTRACT_FILE")
        .get("abi")
        .expect("couldn't get abi from FORCED_EXIT_CONTRACT_FILE")
        .to_string();
    Contract::load(abi_string.as_bytes()).expect("forced_exit contract abi")
}
