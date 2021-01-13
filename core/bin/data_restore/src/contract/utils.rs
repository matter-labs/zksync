use web3::types::Transaction;

use zksync_crypto::params::{INPUT_DATA_ADDRESS_BYTES_WIDTH, INPUT_DATA_ROOT_HASH_BYTES_WIDTH};
use zksync_types::Account;

use crate::eth_tx_helpers::get_input_data_from_ethereum_transaction;

/// Returns Rollup genesis (fees) account from the input of the Rollup contract creation transaction
///
/// # Arguments
///
/// * `transaction` - Ethereum Rollup contract creation transaction description
///
pub fn get_genesis_account(genesis_transaction: &Transaction) -> Result<Account, anyhow::Error> {
    const ENCODED_INIT_PARAMETERS_WIDTH: usize =
        6 * INPUT_DATA_ADDRESS_BYTES_WIDTH + INPUT_DATA_ROOT_HASH_BYTES_WIDTH;

    let input_data = get_input_data_from_ethereum_transaction(&genesis_transaction)?;

    // Input for contract constructor contains the bytecode of the contract and
    // encoded arguments after it.
    // We are not interested in the bytecode and we know the size of arguments,
    // so we can simply cut the parameters bytes from the end of input array,
    // and then decode them to access required data.
    let encoded_init_parameters =
        input_data[input_data.len() - ENCODED_INIT_PARAMETERS_WIDTH..].to_vec();

    // Constructor parameters:
    // ```
    // constructor(
    //    Governance _govTarget, Verifier _verifierTarget, ZkSync _zkSyncTarget,
    //    bytes32 _genesisRoot, address _firstValidator, address _governor,
    //    address _feeAccountAddress
    // )
    let init_parameters_types = vec![
        ethabi::ParamType::Address, // Governance contract address
        ethabi::ParamType::Address, // Verifier contract address
        ethabi::ParamType::Address, // zkSync contract address
        ethabi::ParamType::FixedBytes(INPUT_DATA_ROOT_HASH_BYTES_WIDTH), // Genesis root
        ethabi::ParamType::Address, // First validator (committer) address
        ethabi::ParamType::Address, // Governor address
        ethabi::ParamType::Address, // Fee account address
    ];
    let fee_account_address_argument_id = 6;

    let decoded_init_parameters = ethabi::decode(
        init_parameters_types.as_slice(),
        encoded_init_parameters.as_slice(),
    )
    .map_err(|_| {
        anyhow::Error::from(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "can't get decoded init parameters from contract creation transaction",
        )))
    })?;
    match &decoded_init_parameters[fee_account_address_argument_id] {
        ethabi::Token::Address(genesis_operator_address) => {
            Some(Account::default_with_address(&genesis_operator_address))
        }
        _ => None,
    }
    .ok_or(Err("Invalid token in parameters"))
    .map_err(|_: Result<Account, _>| {
        anyhow::Error::from(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "can't get decoded init parameter from contract creation transaction",
        )))
    })
}
