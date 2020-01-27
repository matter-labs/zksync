use ethabi::Contract;
use std::str::FromStr;

// const ZKSYNC_CONTRACT: &str = include_str!("../../../contracts/build/Franklin.json");
// const GOVERNANCE_CONTRACT: &str = include_str!("../../../contracts/build/Governance.json");
// const PRIORITY_QUEUE_CONTRACT: &str = include_str!("../../../contracts/build/PriorityQueue.json");
// const IERC20_CONTRACT: &str = include_str!("../../../contracts/build/IERC20.json");

const ZKSYNC_CONTRACT: &str = "";
const GOVERNANCE_CONTRACT: &str = "";
const PRIORITY_QUEUE_CONTRACT: &str = "";
const IERC20_CONTRACT: &str = "";


pub fn zksync_contract() -> Contract {
    let abi_string = serde_json::Value::from_str(ZKSYNC_CONTRACT)
        .unwrap()
        .get("abi")
        .unwrap()
        .to_string();
    Contract::load(abi_string.as_bytes()).expect("zksync contract abi")
}

pub fn governance_contract() -> Contract {
    let abi_string = serde_json::Value::from_str(GOVERNANCE_CONTRACT)
        .unwrap()
        .get("abi")
        .unwrap()
        .to_string();
    Contract::load(abi_string.as_bytes()).expect("governance contract abi")
}

pub fn priority_queue_contract() -> Contract {
    let abi_string = serde_json::Value::from_str(PRIORITY_QUEUE_CONTRACT)
        .unwrap()
        .get("abi")
        .unwrap()
        .to_string();
    Contract::load(abi_string.as_bytes()).expect("priority queue contract abi")
}

pub fn erc20_contract() -> Contract {
    let abi_string = serde_json::Value::from_str(IERC20_CONTRACT)
        .unwrap()
        .get("abi")
        .unwrap()
        .to_string();
    Contract::load(abi_string.as_bytes()).expect("erc20 contract abi")
}
