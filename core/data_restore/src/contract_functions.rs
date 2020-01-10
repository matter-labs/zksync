use crate::eth_tx_helpers::get_input_data_from_ethereum_transaction;
use failure::format_err;
use models::node::account::{Account, AccountAddress};
use models::params::{FR_ADDRESS_LEN, INPUT_DATA_ROOT_HASH_BYTES_WIDTH};
use web3::types::Transaction;

/// Returns Rollup genesis (fees) account from the input of the Rollup contract creation transaction
///
/// # Arguments
///
/// * `transaction` - Ethereum Rollup contract creation transaction description
///
pub fn get_genesis_account(genesis_transaction: &Transaction) -> Result<Account, failure::Error> {
    let input_data = get_input_data_from_ethereum_transaction(&genesis_transaction)?;
    let genesis_operator_address = AccountAddress::from_bytes(
        &input_data[input_data.len() - INPUT_DATA_ROOT_HASH_BYTES_WIDTH - FR_ADDRESS_LEN
            ..input_data.len() - INPUT_DATA_ROOT_HASH_BYTES_WIDTH],
    )
    .map_err(|e| format_err!("No genesis account address: {}", e.to_string()))?;
    let mut acc = Account::default();
    acc.address = genesis_operator_address;
    Ok(acc)
}

// /// Returns total number of verified blocks on Rollup contract
// ///
// /// # Arguments
// ///
// /// * `web3` - Web3 provider url
// /// * `franklin_contract` - Rollup contract
// ///
// pub fn get_total_verified_blocks<T: Transport>(
//     web3: &Web3<T>,
//     franklin_contract: &(ethabi::Contract, Contract<T>)
// ) -> u32 {
//     let result = franklin_contract.1.query("totalBlocksVerified", (), None, Options::default(), None);
//     let blocks: U256 = result.wait().unwrap();
//     32
// }
