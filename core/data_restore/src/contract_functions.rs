extern crate ethabi;
use crate::eth_tx_helpers::get_input_data_from_ethereum_transaction;
use models::node::account::Account;
use models::params::{INPUT_DATA_ADDRESS_BYTES_WIDTH, INPUT_DATA_ROOT_HASH_BYTES_WIDTH};
use web3::contract::{Contract, Options};
use web3::futures::Future;
use web3::types::{Address, BlockNumber, Transaction, U256};
use web3::Transport;

/// Returns Rollup genesis (fees) account from the input of the Rollup contract creation transaction
///
/// # Arguments
///
/// * `transaction` - Ethereum Rollup contract creation transaction description
///
pub fn get_genesis_account(genesis_transaction: &Transaction) -> Result<Account, failure::Error> {
    const ENCODED_INIT_PARAMETERS_WIDTH: usize =
        3 * INPUT_DATA_ADDRESS_BYTES_WIDTH + INPUT_DATA_ROOT_HASH_BYTES_WIDTH;

    let input_data = get_input_data_from_ethereum_transaction(&genesis_transaction)?;

    // encoded target initialization parameters
    let encoded_init_parameters =
        input_data[input_data.len() - ENCODED_INIT_PARAMETERS_WIDTH..].to_vec();

    let init_parameters_types = vec![
        ethabi::ParamType::Address,
        ethabi::ParamType::Address,
        ethabi::ParamType::Address,
        ethabi::ParamType::FixedBytes(INPUT_DATA_ROOT_HASH_BYTES_WIDTH),
    ];
    let decoded_init_parameters = ethabi::decode(
        init_parameters_types.as_slice(),
        encoded_init_parameters.as_slice(),
    )
    .map_err(|_| {
        failure::Error::from_boxed_compat(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "can't get decoded init parameters from contract creation transaction",
        )))
    })?;
    match &decoded_init_parameters[2] {
        ethabi::Token::Address(genesis_operator_address) => {
            Some(Account::default_with_address(&genesis_operator_address))
        }
        _ => None,
    }
    .ok_or_else(|| Err("Invalid token in parameters"))
    .map_err(|_: Result<Account, _>| {
        failure::Error::from_boxed_compat(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "can't get decoded init parameter from contract creation transaction",
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
