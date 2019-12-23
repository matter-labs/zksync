// External uses
use failure::{ensure, format_err};
use web3::futures::Future;
use web3::types::H256;
use web3::types::{Transaction, TransactionId};

pub const FUNC_NAME_HASH_LENGTH: usize = 4;

/// Return Ethereum transaction input data
///
/// # Arguments
///
/// * `transaction` - Ethereum transaction description
///
pub fn get_input_data_from_ethereum_transaction(
    transaction: &Transaction,
) -> Result<Vec<u8>, failure::Error> {
    let input_data = transaction.clone().input.0;
    // info!("Start input: {:?}", &input_data);
    ensure!(
        input_data.len() > FUNC_NAME_HASH_LENGTH,
        format_err!("No commitment data in tx")
    );
    Ok(input_data[FUNC_NAME_HASH_LENGTH..input_data.len()].to_vec())
}

/// Return Ethereum transaction input data
///
/// # Arguments
///
/// * `transaction` - Ethereum transaction description
///
pub fn get_block_number_from_ethereum_transaction(
    transaction: &Transaction,
) -> Result<u64, failure::Error> {
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
/// * `transaction_hash` - The identifier of the particular Ethereum transaction
///
pub fn get_ethereum_transaction(
    web3_url: &String,
    transaction_hash: &H256,
) -> Result<Transaction, failure::Error> {
    let (_eloop, transport) = web3::transports::Http::new(web3_url).unwrap();
    let web3 = web3::Web3::new(transport);
    let tx_id = TransactionId::Hash(*transaction_hash);
    let web3_transaction = web3
        .eth()
        .transaction(tx_id)
        .wait()
        .map_err(|e| format_err!("No response from web3: {}", e.to_string()))?
        .ok_or_else(|| format_err!("No tx with this hash"))?;
    Ok(web3_transaction)
}
