use ethabi::{Contract, Param, ParamType};
use std::fs;
use std::io;
use std::str::FromStr;

const ZKSYNC_CONTRACT_FILE: &str =
    "contracts/artifacts/cache/solpp-generated-contracts/ZkSync.sol/ZkSync.json";
const GOVERNANCE_CONTRACT_FILE: &str =
    "contracts/artifacts/cache/solpp-generated-contracts/Governance.sol/Governance.json";
const IERC20_CONTRACT_FILE: &str =
    "contracts/artifacts/cache/solpp-generated-contracts/IERC20.sol/IERC20.json";
const IEIP1271_CONTRACT_FILE: &str =
    "contracts/artifacts/cache/solpp-generated-contracts/IEIP1271.sol/IEIP1271.json";

fn read_file_to_json_value(path: &str) -> io::Result<serde_json::Value> {
    let zksync_home = std::env::var("ZKSYNC_HOME").unwrap_or_else(|_| ".".into());
    let path = std::path::Path::new(&zksync_home).join(path);
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
    let mut contract = Contract::load(abi_string.as_bytes()).expect("zksync contract abi");
    let mut commit_blocks_func = contract
        .functions
        .remove("commitBlocks")
        .into_iter()
        .flatten()
        .next()
        .expect("ZkSync.sol commitBlocks(..) not found.");

    commit_blocks_func.inputs = vec![
        Param {
            name: "_lastCommittedBlockData".to_string(),
            kind: ParamType::Tuple(vec![
                Box::new(ParamType::Uint(32)),
                Box::new(ParamType::Uint(64)),
                Box::new(ParamType::FixedBytes(32)),
                Box::new(ParamType::FixedBytes(32)),
                Box::new(ParamType::FixedBytes(32)),
            ]),
        },
        Param {
            name: "_newBlocksData".to_string(),
            kind: ParamType::Array(Box::new(ParamType::Tuple(vec![
                Box::new(ParamType::Uint(32)),
                Box::new(ParamType::Uint(32)),
                Box::new(ParamType::FixedBytes(32)),
                Box::new(ParamType::Bytes),
                Box::new(ParamType::Array(Box::new(ParamType::Tuple(vec![
                    Box::new(ParamType::Uint(32)),
                    Box::new(ParamType::Bytes),
                ])))),
            ]))),
        },
    ];

    contract
        .functions
        .insert("commitBlocks".to_string(), vec![commit_blocks_func]);

    contract
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
