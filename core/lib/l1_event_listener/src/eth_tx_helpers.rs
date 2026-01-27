// External uses
use anyhow::{ensure, format_err};
use web3::types::H256;
use web3::types::{Transaction, TransactionId};
use web3::{Transport, Web3};

pub const FUNC_NAME_HASH_LENGTH: usize = 4;

/// Returns Ethereum transaction input data
///
/// # Arguments
///
/// * `transaction` - Ethereum transaction description
///
pub fn get_input_data_from_ethereum_transaction(
    transaction: &Transaction,
) -> Result<Vec<u8>, anyhow::Error> {
    let input_data = transaction.clone().input.0;
    ensure!(
        input_data.len() > FUNC_NAME_HASH_LENGTH,
        format_err!("No commitment data in tx")
    );
    Ok(input_data[FUNC_NAME_HASH_LENGTH..].to_vec())
}

/// Returns Ethereum transaction block number
///
/// # Arguments
///
/// * `transaction` - Ethereum transaction description
///
pub fn get_block_number_from_ethereum_transaction(
    transaction: &Transaction,
) -> Result<u64, anyhow::Error> {
    Ok(transaction
        .clone()
        .block_number
        .ok_or_else(|| format_err!("No block number info in tx"))?
        .as_u64())
}

/// Return Ethereum transaction description
///
/// # Arguments
///
/// * `web3` - Web3 provider url
/// * `transaction_hash` - The identifier of the particular Ethereum transaction
///
pub async fn get_ethereum_transaction<T: Transport>(
    web3: &Web3<T>,
    transaction_hash: &H256,
) -> Result<Transaction, anyhow::Error> {
    let tx_id = TransactionId::Hash(*transaction_hash);
    let web3_transaction = web3
        .eth()
        .transaction(tx_id)
        .await
        .map_err(|e| format_err!("No response from web3: {}", e))?
        .ok_or_else(|| format_err!("No tx with this hash"))?;
    Ok(web3_transaction)
}
