// TODO: Temporary, to not have all the code completely yellow.
#![allow(dead_code, unused_imports, unused_variables, unused_mut)]

use super::rpc_server::types::{
    BlockInfo, ETHOpInfoResp, ResponseAccountState, TransactionInfoResp,
};
use crate::utils::token_db_cache::TokenDBCache;
use anyhow::{bail, format_err};
use futures::{
    channel::{mpsc, oneshot},
    compat::Future01CompatExt,
    select,
    stream::StreamExt,
    FutureExt, SinkExt,
};
use jsonrpc_pubsub::{
    typed::{Sink, Subscriber},
    SubscriptionId,
};
use lru_cache::LruCache;
use std::collections::BTreeMap;
use std::str::FromStr;
use zksync_basic_types::Address;
use zksync_storage::chain::operations::records::StoredExecutedPriorityOperation;
use zksync_storage::chain::operations_ext::records::TxReceiptResponse;
use zksync_storage::ConnectionPool;
use zksync_types::tx::TxHash;
use zksync_types::BlockNumber;
use zksync_types::{block::ExecutedOperations, AccountId, ActionType, Operation};

use self::operation_notifier::OperationNotifier;

mod event_notify_fetcher;
mod operation_notifier;
mod state;
mod sub_store;

pub struct ExecutedOpsNotify {
    pub operations: Vec<ExecutedOperations>,
    pub block_number: BlockNumber,
}

pub enum ExecutedOpId {
    Transaction(TxHash),
    PriorityOp(u64),
}

pub enum EventSubscribeRequest {
    Transaction {
        hash: TxHash,
        action: ActionType,
        subscriber: Subscriber<TransactionInfoResp>,
    },
    PriorityOp {
        serial_id: u64,
        action: ActionType,
        subscriber: Subscriber<ETHOpInfoResp>,
    },
    Account {
        address: Address,
        action: ActionType,
        subscriber: Subscriber<ResponseAccountState>,
    },
}

pub enum EventNotifierRequest {
    Sub(EventSubscribeRequest),
    Unsub(SubscriptionId),
}

struct SubscriptionSender<T> {
    id: SubscriptionId,
    sink: Sink<T>,
}

pub fn start_sub_notifier(
    db_pool: ConnectionPool,
    mut new_block_stream: mpsc::Receiver<Operation>,
    mut subscription_stream: mpsc::Receiver<EventNotifierRequest>,
    api_requests_caches_size: usize,
) -> tokio::task::JoinHandle<()> {
    let tokens_cache = TokenDBCache::new(db_pool.clone());

    let mut notifier = OperationNotifier {
        cache_of_executed_priority_operations: LruCache::new(api_requests_caches_size),
        cache_of_transaction_receipts: LruCache::new(api_requests_caches_size),
        cache_of_blocks_info: LruCache::new(api_requests_caches_size),
        tokens_cache,
        db_pool,
        tx_subs: BTreeMap::new(),
        prior_op_subs: BTreeMap::new(),
        account_subs: BTreeMap::new(),
    };

    tokio::spawn(async move {
        loop {
            select! {
                // new_block = new_block_stream.next() => {
                //     if let Some(new_block) = new_block {
                //         notifier.handle_new_block(new_block)
                //             .await
                //             .map_err(|e| log::warn!("Failed to handle new block: {}",e))
                //             .unwrap_or_default();
                //     }
                // },
                // new_exec_batch = executed_tx_stream.next() => {
                //     if let Some(new_exec_batch) = new_exec_batch {
                //         notifier.handle_new_executed_batch(new_exec_batch)
                //             .map_err(|e| log::warn!("Failed to handle new exec batch: {}",e))
                //             .unwrap_or_default();
                //     }
                // },
                new_sub = subscription_stream.next() => {
                    if let Some(new_sub) = new_sub {
                        notifier.handle_notify_req(new_sub)
                            .await
                            .map_err(|e| log::warn!("Failed to handle notify request: {}",e))
                            .unwrap_or_default();
                    }
                },
                complete => break,
            }
        }
    })
}
