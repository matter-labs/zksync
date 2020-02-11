use ethabi::Contract;
use std::fs;
use std::io;
use std::str::FromStr;

const ZKSYNC_CONTRACT_FILE: &str = "contracts/build/Franklin.json";
const GOVERNANCE_CONTRACT_FILE: &str = "contracts/build/Governance.json";
const PRIORITY_QUEUE_CONTRACT_FILE: &str = "contracts/build/PriorityQueue.json";
const IERC20_CONTRACT_FILE: &str = "contracts/build/IERC20.json";

fn read_file_to_json_value(path: &str) -> io::Result<serde_json::Value> {
    let contents = fs::read_to_string(path)?;
    let val = serde_json::Value::from_str(&contents)?;
    Ok(val)
}

pub fn zksync_contract() -> Contract {
    let abi_string = read_file_to_json_value(ZKSYNC_CONTRACT_FILE)
        .expect("couldn't read ZKSYNC_CONTRACT_FILE")
        .get("abi")
        .expect("couldn't get abi from ZKSYNC_CONTRACT_FILE")
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

pub fn priority_queue_contract() -> Contract {
    let abi_string = read_file_to_json_value(PRIORITY_QUEUE_CONTRACT_FILE)
        .expect("couldn't read PRIORITY_QUEUE_CONTRACT_FILE")
        .get("abi")
        .expect("couldn't get abi from PRIORITY_QUEUE_CONTRACT_FILE")
        .to_string();
    Contract::load(abi_string.as_bytes()).expect("priority queue contract abi")
}

pub fn erc20_contract() -> Contract {
    let abi_string = read_file_to_json_value(IERC20_CONTRACT_FILE)
        .expect("couldn't read IERC20_CONTRACT_FILE")
        .get("abi")
        .expect("couldn't get abi from IERC20_CONTRACT_FILE")
        .to_string();
    Contract::load(abi_string.as_bytes()).expect("erc20 contract abi")
}
