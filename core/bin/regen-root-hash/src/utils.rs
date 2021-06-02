use ethabi::Contract;
use std::fs;
use std::io;
use std::str::FromStr;
use zksync_crypto::{
    ff::{PrimeField, PrimeFieldRepr},
    Fr,
};

const REGEN_MULTISIG_CONTRACT: &str =
    "contracts/artifacts/cache/solpp-generated-contracts/RegenesisMultisig.sol/RegenesisMultisig.json";

fn read_file_to_json_value(path: &str) -> io::Result<serde_json::Value> {
    let zksync_home = std::env::var("ZKSYNC_HOME").unwrap_or_else(|_| ".".into());
    println!("{}", zksync_home);
    let path = std::path::Path::new(&zksync_home).join(path);
    let contents = fs::read_to_string(path).unwrap();
    let val = serde_json::Value::from_str(&contents).unwrap();
    Ok(val)
}

pub fn fr_to_bytes(scalar: Fr) -> Vec<u8> {
    let mut be_bytes = [0u8; 32];
    scalar
        .into_repr()
        .write_be(be_bytes.as_mut())
        .expect("Write commit bytes");

    be_bytes.to_vec()
}

pub fn fr_to_hex(scalar: Fr) -> String {
    let be_bytes = fr_to_bytes(scalar);

    hex::encode(be_bytes)
}

pub fn regen_multisig_contract() -> Contract {
    let abi_string = read_file_to_json_value(REGEN_MULTISIG_CONTRACT)
        .expect("couldn't read REGEN_MULTISIG_CONTRACT")
        .get("abi")
        .expect("couldn't get abi from REGEN_MULTISIG_CONTRACT")
        .to_string();
    Contract::load(abi_string.as_bytes()).expect("regenesis multiisg contract abi")
}

// Returns hex-encoded tx data for contract call
pub fn get_tx_data(old_hash: Fr, new_hash: Fr) -> String {
    let regen_multisig_contract = regen_multisig_contract();

    let function = regen_multisig_contract
        .function("submitHash")
        .expect("no submitHash function");

    let input_tokens = vec![
        ethabi::Token::FixedBytes(fr_to_bytes(old_hash)),
        ethabi::Token::FixedBytes(fr_to_bytes(new_hash)),
    ];

    let calldata = function
        .encode_input(&input_tokens)
        .expect("Failed to encode bytes");

    hex::encode(&calldata)
}
