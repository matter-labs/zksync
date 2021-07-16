// Built-in uses
use std::convert::TryInto;
// External uses
use jsonrpc_core::{Error, Result};
use num::BigUint;
// Workspace uses
use zksync_storage::{chain::operations_ext::records::Web3TxReceipt, StorageProcessor};
// Local uses
use super::types::{
    Bytes, CommonLogData, Log, Transaction, TransactionReceipt, TxData, H160, H2048, H256, U256,
    U64,
};

pub fn u256_from_biguint(number: BigUint) -> Result<U256> {
    U256::from_dec_str(&number.to_string()).map_err(|_| Error::internal_error())
}

pub async fn resolve_block_number(
    storage: &mut StorageProcessor<'_>,
    number: Option<super::types::BlockNumber>,
) -> Result<Option<zksync_types::BlockNumber>> {
    let last_saved_block = storage
        .chain()
        .block_schema()
        .get_last_saved_block()
        .await
        .map_err(|_| Error::internal_error())?;

    let number = match number {
        Some(number) => number,
        None => {
            return Ok(Some(last_saved_block));
        }
    };

    let number = match number {
        super::types::BlockNumber::Earliest => zksync_types::BlockNumber(0),
        super::types::BlockNumber::Committed => storage
            .chain()
            .block_schema()
            .get_last_committed_confirmed_block()
            .await
            .map_err(|_| Error::internal_error())?,
        super::types::BlockNumber::Finalized => storage
            .chain()
            .block_schema()
            .get_last_verified_confirmed_block()
            .await
            .map_err(|_| Error::internal_error())?,
        super::types::BlockNumber::Latest | super::types::BlockNumber::Pending => last_saved_block,
        super::types::BlockNumber::Number(number) => {
            if number.as_u64() > last_saved_block.0 as u64 {
                return Ok(None);
            }
            // Unwrap can be safely used because `number` is not greater than `last_saved_block`
            // which is `u32` variable.
            zksync_types::BlockNumber(number.as_u64().try_into().unwrap())
        }
    };
    Ok(Some(number))
}

pub fn transaction_from_tx_data(tx: TxData) -> Transaction {
    Transaction {
        hash: tx.tx_hash,
        nonce: tx.nonce.into(),
        block_hash: tx.block_hash,
        block_number: tx.block_number.map(Into::into),
        transaction_index: tx.block_index.map(Into::into),
        from: tx.from,
        to: tx.to,
        value: 0.into(),
        gas_price: 0.into(),
        gas: 0.into(),
        input: Vec::new().into(),
        raw: None,
    }
}

pub fn tx_receipt_from_storage_receipt(tx: Web3TxReceipt) -> TransactionReceipt {
    let root_hash = H256::from_slice(&tx.block_hash);
    TransactionReceipt {
        transaction_hash: H256::from_slice(&tx.tx_hash),
        // U64::MAX for failed transactions
        transaction_index: tx.block_index.map(Into::into).unwrap_or(U64::MAX),
        block_hash: Some(root_hash),
        block_number: Some(tx.block_number.into()),
        cumulative_gas_used: 0.into(),
        gas_used: Some(0.into()),
        contract_address: None,
        logs: Vec::new(),
        status: Some((tx.success as u8).into()),
        root: Some(root_hash),
        logs_bloom: H2048::zero(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn log(
    address: H160,
    topic: H256,
    data: Bytes,
    common_data: CommonLogData,
    transaction_log_index: U256,
) -> Log {
    Log {
        address,
        topics: vec![topic],
        data,
        block_hash: common_data.block_hash,
        block_number: common_data.block_number,
        transaction_hash: Some(common_data.transaction_hash),
        transaction_index: common_data.transaction_index,
        log_index: None,
        transaction_log_index: Some(transaction_log_index),
        log_type: None,
        removed: Some(false),
    }
}
