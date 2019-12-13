use crate::helpers::{
    get_input_data_from_ethereum_transaction,
};
use failure::format_err;
use models::node::{
    account::{Account, AccountAddress},
    AccountMap,
};
use web3::types::Transaction;
use models::params::{FR_ADDRESS_LEN, INPUT_DATA_ROOT_HASH_BYTES_WIDTH};

// Returns contracts genesis accounts state
pub fn get_genesis_state(genesis_transaction: &Transaction) -> Result<(u32, AccountMap), failure::Error> {
    let input_data = get_input_data_from_ethereum_transaction(&genesis_transaction)?;
    let genesis_operator_address = AccountAddress::from_bytes(
        &input_data[input_data.len() - INPUT_DATA_ROOT_HASH_BYTES_WIDTH - FR_ADDRESS_LEN
            ..input_data.len() - INPUT_DATA_ROOT_HASH_BYTES_WIDTH],
    )
    .map_err(|e| format_err!("No genesis account address: {}", e.to_string()))?;
    let mut acc = Account::default();
    acc.address = genesis_operator_address;
    let mut map = AccountMap::default();
    map.insert(0, acc);
    Ok((0, map))
}
