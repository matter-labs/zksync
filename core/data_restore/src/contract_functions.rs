extern crate ethabi;
use crate::eth_tx_helpers::get_input_data_from_ethereum_transaction;
use models::node::account::Account;
use models::params::INPUT_DATA_ROOT_HASH_BYTES_WIDTH;
use web3::contract::{Contract, Options};
use web3::futures::Future;
use web3::types::{Address, BlockNumber, Transaction, U256};
use web3::Transport;

/// Returns Rollup genesis (fees) account from the input of the Rollup contract initialization transaction
///
/// # Arguments
///
/// * `transaction` - Ethereum Rollup contract initialization transaction description
///
pub fn get_genesis_account(genesis_transaction: &Transaction) -> Result<Account, failure::Error> {
    let input_data = get_input_data_from_ethereum_transaction(&genesis_transaction)?;

    // target address and targetInitializationParameters
    let input_parameters = ethabi::decode(
        vec![ethabi::ParamType::Address, ethabi::ParamType::Bytes].as_slice(),
        input_data.as_slice(),
    )
    .map_err(|_| {
        failure::Error::from_boxed_compat(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "can't get input parameters from target initialization transaction",
        )))
    })?;
    let encoded_parameters = input_parameters[1]
        .clone()
        .to_bytes()
        .ok_or_else(|| Err("Invalid token in parameters"))
        .map_err(|_: Result<Vec<u8>, _>| {
            failure::Error::from_boxed_compat(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "can't get initialization parameters from target initialization transaction",
            )))
        })?;

    let input_types = vec![
        ethabi::ParamType::Address,
        ethabi::ParamType::Address,
        ethabi::ParamType::Address,
        ethabi::ParamType::FixedBytes(INPUT_DATA_ROOT_HASH_BYTES_WIDTH),
    ];
    let decoded_parameters = ethabi::decode(input_types.as_slice(), encoded_parameters.as_slice())
        .map_err(|_| {
            failure::Error::from_boxed_compat(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "can't get decoded parameters from target initialization transaction",
            )))
        })?;
    match &decoded_parameters[2] {
        ethabi::Token::Address(genesis_operator_address) => {
            Some(Account::default_with_address(&genesis_operator_address))
        }
        _ => None,
    }
    .ok_or_else(|| Err("Invalid token in parameters"))
    .map_err(|_: Result<Account, _>| {
        failure::Error::from_boxed_compat(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "can't get decoded parameter from target initialization transaction",
        )))
    })
}

/// Returns total number of verified blocks on Rollup contract
///
/// # Arguments
///
/// * `web3` - Web3 provider url
/// * `franklin_contract` - Rollup contract
///
pub fn get_total_verified_blocks<T: Transport>(
    franklin_contract: &(ethabi::Contract, Contract<T>),
) -> u32 {
    franklin_contract
        .1
        .query::<U256, Option<Address>, Option<BlockNumber>, ()>(
            "totalBlocksVerified",
            (),
            None,
            Options::default(),
            None,
        )
        .wait()
        .unwrap()
        .as_u32()
}
