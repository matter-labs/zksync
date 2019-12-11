// External uses
use failure::format_err;
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
    if input_data.len() > FUNC_NAME_HASH_LENGTH {
        Ok(input_data[FUNC_NAME_HASH_LENGTH..input_data.len()].to_vec())
    } else {
        Err(format_err!("No commitment data in tx"))
    }
}

/// Return Ethereum transaction description
///
/// # Arguments
///
/// * `transaction_hash` - The identifier of the particular Ethereum transaction
///
pub fn get_ethereum_transaction(transaction_hash: &H256) -> Result<Transaction, failure::Error> {
    let tx_id = TransactionId::Hash(*transaction_hash);
    let (_eloop, transport) =
        web3::transports::Http::new(DATA_RESTORE_CONFIG.web3_endpoint.as_str())
            .map_err(|e| format_err!("Wrong endpoint: {}", e.to_string()))?;
    let web3 = web3::Web3::new(transport);
    let web3_transaction = web3
        .eth()
        .transaction(tx_id)
        .wait()
        .map_err(|e| format_err!("No response from web3: {}", e.to_string()))?
        .ok_or_else(|| format_err!("No tx with this hash"))?;
    Ok(web3_transaction)
}
