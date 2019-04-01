use super::{DepositBlock, TransferBlock, ExitBlock};
use plasma::circuit::utils::be_bit_vector_into_bytes;
use plasma::models;

pub fn encode_transfer_transactions(block: &TransferBlock) -> Result<Vec<u8>, String> {
    let mut encoding: Vec<u8> = vec![];

    let transactions = &block.transactions;

    for tx in transactions {
        let tx = models::circuit::TransferTx::try_from(tx).map_err(|e| e.to_string())?; // BabyProverErr::InvalidTransaction(e.to_string())
        let tx_bits = tx.public_data_into_bits();
        let tx_encoding = be_bit_vector_into_bytes(&tx_bits);
        encoding.extend(tx_encoding.into_iter());
    }

    Ok(encoding)
}

pub fn encode_deposit_transactions(block: &DepositBlock) -> Result<Vec<u8>, String> {
    let mut encoding: Vec<u8> = vec![];

    // let sorted_block = sorted_deposit_block(&block);

    let transactions = &block.transactions;

    for tx in transactions {
        let tx = models::circuit::DepositRequest::try_from(tx).map_err(|e| e.to_string())?;
        let tx_bits = tx.public_data_into_bits();
        let tx_encoding = be_bit_vector_into_bytes(&tx_bits);
        encoding.extend(tx_encoding.into_iter());
    }

    Ok(encoding)
}

// this method is different, it actually reads the state 
pub fn encode_exit_transactions(block: &ExitBlock) -> Result<Vec<u8>, String> {
    let mut encoding: Vec<u8> = vec![];

    let transactions = &block.transactions;

    for tx in transactions {
        let tx = models::circuit::ExitRequest::try_from(tx).map_err(|e| e.to_string())?;
        // if tx.amount == Fr::zero() {
        //     println!("Trying to exit a zero balance");
        // }
        let tx_bits = tx.public_data_into_bits();
        let tx_encoding = be_bit_vector_into_bytes(&tx_bits);
        encoding.extend(tx_encoding.into_iter());
    }

    // let public_data_hex: String = encoding.clone().to_hex();
    // println!("Final encoding = {}", public_data_hex);

    Ok(encoding)
}
