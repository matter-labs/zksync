use crate::helpers::{
    get_ethereum_transaction, get_input_data_from_ethereum_transaction, DataRestoreError,
    DATA_RESTORE_CONFIG,
};
use models::node::{
    account::{Account, AccountAddress},
    AccountMap,
};
use models::params::FR_ADDRESS_LEN;

const ROOT_HASH_LENGTH: usize = 32;

// Returns contracts genesis accounts state
pub fn get_genesis_state() -> Result<(u32, AccountMap), DataRestoreError> {
    let genesis_tx_hash = DATA_RESTORE_CONFIG.genesis_tx_hash;
    let transaction = get_ethereum_transaction(&genesis_tx_hash)?;
    let input_data = get_input_data_from_ethereum_transaction(&transaction)?;
    let genesis_operator_address = AccountAddress::from_bytes(
        &input_data[input_data.len() - ROOT_HASH_LENGTH - FR_ADDRESS_LEN
            ..input_data.len() - ROOT_HASH_LENGTH],
    )
    .map_err(|err| DataRestoreError::WrongData(err.to_string()))?;
    let mut acc = Account::default();
    acc.address = genesis_operator_address;
    let mut map = AccountMap::default();
    map.insert(0, acc);
    Ok((0, map))
}
