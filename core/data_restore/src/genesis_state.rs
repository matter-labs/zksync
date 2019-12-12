use crate::helpers::{
    get_input_data_from_ethereum_transaction,
};
use failure::format_err;
use models::node::{
    account::{Account, AccountAddress},
    AccountMap,
};
use web3::types::{Transaction, TransactionId};
use models::params::FR_ADDRESS_LEN;
use web3::{Transport, Web3};
use web3::types::{Address, BlockNumber, Filter, FilterBuilder, Log, H160, U256};

const ROOT_HASH_LENGTH: usize = 32;

// Returns contracts genesis accounts state
pub fn get_genesis_state(genesis_transaction: &Transaction) -> Result<(u32, AccountMap), failure::Error> {
    let input_data = get_input_data_from_ethereum_transaction(&genesis_transaction)?;
    let genesis_operator_address = AccountAddress::from_bytes(
        &input_data[input_data.len() - ROOT_HASH_LENGTH - FR_ADDRESS_LEN
            ..input_data.len() - ROOT_HASH_LENGTH],
    )
    .map_err(|e| format_err!("No genesis account address: {}", e.to_string()))?;
    let mut acc = Account::default();
    acc.address = genesis_operator_address;
    let mut map = AccountMap::default();
    map.insert(0, acc);
    Ok((0, map))
}
