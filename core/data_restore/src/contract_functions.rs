extern crate ethabi;
use crate::eth_tx_helpers::get_input_data_from_ethereum_transaction;
use models::node::account::Account;
use models::params::{INPUT_DATA_ROOT_HASH_BYTES_WIDTH};
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
    // encoded target address and targetInitializationParameters
    let input_data = get_input_data_from_ethereum_transaction(&genesis_transaction)?;
    // encoded targetInitializationParameters
    let encoded_parameters;
    if let Ok(parameters) = ethabi::decode(vec![ethabi::ParamType::Address, ethabi::ParamType::Bytes].as_slice(), input_data.as_slice()) {
        if let ethabi::Token::Bytes(parameters) = &parameters.clone()[1] {
            encoded_parameters = (*parameters).clone().to_vec();
        }
        else {
            return Result::Err(std::io::Error::new(std::io::ErrorKind::NotFound, "can't get encoded parameters from target initialization transaction").into());
        }
    }
    else{
        return Result::Err(std::io::Error::new(std::io::ErrorKind::NotFound, "can't get encoded parameters from target initialization transaction").into());
    }
    let input_types = vec![
        ethabi::ParamType::Address,
        ethabi::ParamType::Address,
        ethabi::ParamType::Address,
        ethabi::ParamType::FixedBytes(INPUT_DATA_ROOT_HASH_BYTES_WIDTH),
    ];
    let decoded_parameters;
    if let Ok(parameters) = ethabi::decode(input_types.as_slice(), encoded_parameters.as_slice()) {
        decoded_parameters = parameters;
    }
    else{
        return Result::Err(std::io::Error::new(std::io::ErrorKind::NotFound, "can't get decode parameters of initialiation").into());
    }
    if let Some(ethabi::Token::Address(genesis_operator_address)) = decoded_parameters.get(2) {
        Ok(Account::default_with_address(&genesis_operator_address))
    }
    else{
        Result::Err(std::io::Error::new(std::io::ErrorKind::NotFound, "can't get genesis operator address from decoded parameters").into())
    }
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
