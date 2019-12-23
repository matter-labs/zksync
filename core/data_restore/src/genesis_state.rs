use crate::helpers::get_input_data_from_ethereum_transaction;
use failure::format_err;
use models::node::account::{Account, AccountAddress};
use models::params::{FR_ADDRESS_LEN, INPUT_DATA_ROOT_HASH_BYTES_WIDTH};
use web3::types::Transaction;

// Returns contracts genesis accounts state
pub fn get_genesis_account(
    genesis_transaction: &Transaction,
) -> Result<Account, failure::Error> {
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

pub fn get_tokens() -> Result<Vec<(u16, String, Option<String>)>, failure::Error> {
    return Ok(vec![(1, "0x54FCb2405EE4f574C4F09333d25c401E68aD3408".to_string(), None)])
}
