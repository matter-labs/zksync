//! Utilities for the REST API.

use crate::core_api_client::EthBlockId;
use actix_web::{HttpResponse, Result as ActixResult};
use std::collections::HashMap;
use zksync_storage::chain::{
    block::records::BlockDetails,
    operations_ext::records::{TransactionsHistoryItem, TxByHashResponse},
};
use zksync_storage::StorageProcessor;
use zksync_types::{PriorityOp, Token, TokenId, ZkSyncPriorityOp, H256};

pub fn remove_prefix(query: &str) -> &str {
    if query.starts_with("0x") {
        &query[2..]
    } else if query.starts_with("sync-bl:") || query.starts_with("sync-tx:") {
        &query[8..]
    } else {
        &query
    }
}

pub fn try_parse_hash(query: &str) -> Option<H256> {
    const HASH_SIZE: usize = 32; // 32 bytes

    let query = remove_prefix(query);
    let bytes = hex::decode(query).ok()?;

    if bytes.len() == HASH_SIZE {
        Some(H256::from_slice(&bytes))
    } else {
        None
    }
}

/// Checks if block is finalized, meaning that
/// both Verify operation is performed for it, and this
/// operation is anchored on the Ethereum blockchain.
pub fn block_verified(block: &BlockDetails) -> bool {
    // We assume that it's not possible to have block that is
    // verified and not committed.
    block.verified_at.is_some() && block.verify_tx_hash.is_some()
}

/// Converts a non-executed priority operation into a
/// `TxByHashResponse` so the user can track its status in explorer.
/// It also adds new field `tx.eth_block_number`, which is normally not there,
/// which is the block number of Ethereum tx of the priority operation,
/// it enables tracking the number of blocks (confirmations) user needs to wait
/// before the priority op is included into zkSync block.
/// Currently returns Some(TxByHashResponse) if PriorityOp is Deposit, and None in other cases.
pub fn deposit_op_to_tx_by_hash(
    tokens: &HashMap<TokenId, Token>,
    op: &PriorityOp,
    eth_block: EthBlockId,
) -> Option<TxByHashResponse> {
    match &op.data {
        ZkSyncPriorityOp::Deposit(deposit) => {
            // As the time of creation is indefinite, we always will provide the current time.
            let current_time = chrono::Utc::now();
            let naive_current_time =
                chrono::NaiveDateTime::from_timestamp(current_time.timestamp(), 0);

            // Account ID may not exist for depositing ops, so it'll be `null`.
            let account_id: Option<u32> = None;

            let token_symbol = tokens.get(&deposit.token).map(|t| t.symbol.clone());

            // Copy the JSON representation of the executed tx so the appearance
            // will be the same as for txs from storage.
            let tx_json = serde_json::json!({
                "account_id": account_id,
                "priority_op": {
                    "amount": deposit.amount,
                    "from": deposit.from,
                    "to": deposit.to,
                    "token": token_symbol
                },
                "type": "Deposit",
                "eth_block_number": eth_block,
            });

            Some(TxByHashResponse {
                tx_type: "Deposit".into(),
                from: format!("{:?}", deposit.from),
                to: format!("{:?}", deposit.to),
                token: deposit.token as i32,
                amount: deposit.amount.to_string(),
                fee: None,
                block_number: -1,
                nonce: -1,
                created_at: naive_current_time
                    .format("%Y-%m-%dT%H:%M:%S%.6f")
                    .to_string(),
                fail_reason: None,
                tx: tx_json,
            })
        }
        _ => None,
    }
}

/// Converts a non-executed priority operation into a
/// `TransactionsHistoryItem` to include it into the list of transactions
/// in the client.
pub fn priority_op_to_tx_history(
    tokens: &HashMap<TokenId, Token>,
    eth_block: u64,
    op: &PriorityOp,
) -> TransactionsHistoryItem {
    let deposit = op
        .data
        .try_get_deposit()
        .expect("Not a deposit sent by eth_watch");
    let token_symbol = tokens
        .get(&deposit.token)
        .map(|t| t.symbol.clone())
        .unwrap_or_else(|| "unknown".into());

    let hash_str = format!("0x{}", hex::encode(&op.eth_hash));
    let pq_id = Some(op.serial_id as i64);

    // Account ID may not exist for depositing ops, so it'll be `null`.
    let account_id: Option<u32> = None;

    // Copy the JSON representation of the executed tx so the appearance
    // will be the same as for txs from storage.
    let tx_json = serde_json::json!({
        "account_id": account_id,
        "priority_op": {
            "amount": deposit.amount.to_string(),
            "from": deposit.from,
            "to": deposit.to,
            "token": token_symbol
        },
        "type": "Deposit"
    });

    // As the time of creation is indefinite, we always will provide the current time.
    let current_time = chrono::Utc::now();

    TransactionsHistoryItem {
        tx_id: "-".into(),
        hash: Some(hash_str),
        eth_block: Some(eth_block as i64),
        pq_id,
        tx: tx_json,
        success: None,
        fail_reason: None,
        commited: false,
        verified: false,
        created_at: current_time,
    }
}

pub async fn parse_tx_id(
    data: &str,
    storage: &mut StorageProcessor<'_>,
) -> ActixResult<(u64, u64)> {
    if data.is_empty() || data == "-" {
        let last_block_id = storage
            .chain()
            .block_schema()
            .get_last_committed_block()
            .await
            .map_err(|err| {
                vlog::warn!("Internal Server Error: '{}'; input: ({})", err, data,);
                HttpResponse::InternalServerError().finish()
            })?;

        let next_block_id = last_block_id + 1;

        return Ok((next_block_id as u64, 0));
    }

    let parts: Vec<u64> = data
        .split(',')
        .map(|val| val.parse().map_err(|_| HttpResponse::BadRequest().finish()))
        .collect::<Result<Vec<u64>, HttpResponse>>()?;

    if parts.len() != 2 {
        return Err(HttpResponse::BadRequest().finish().into());
    }

    Ok((parts[0], parts[1]))
}
