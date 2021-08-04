// Built-in uses
use std::convert::TryInto;
// External uses
use jsonrpc_core::{Error, Result};
use num::BigUint;
// Workspace uses
use zksync_storage::StorageProcessor;
// Local uses
use super::types::{BlockNumber, Bytes, CommonLogData, Log, Transaction, TxData, H160, H256, U256};

pub fn u256_from_biguint(number: BigUint) -> Result<U256> {
    U256::from_dec_str(&number.to_string()).map_err(|_| Error::internal_error())
}

pub async fn resolve_block_number(
    storage: &mut StorageProcessor<'_>,
    number: Option<BlockNumber>,
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
        BlockNumber::Earliest => zksync_types::BlockNumber(0),
        BlockNumber::Committed => storage
            .chain()
            .block_schema()
            .get_last_committed_confirmed_block()
            .await
            .map_err(|_| Error::internal_error())?,
        BlockNumber::Finalized => storage
            .chain()
            .block_schema()
            .get_last_verified_confirmed_block()
            .await
            .map_err(|_| Error::internal_error())?,
        BlockNumber::Latest | BlockNumber::Pending => last_saved_block,
        BlockNumber::Number(number) => {
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
        block_hash: Some(tx.block_hash),
        block_number: Some(tx.block_number.into()),
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
