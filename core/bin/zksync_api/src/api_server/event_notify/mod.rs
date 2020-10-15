use super::rpc_server::types::{ETHOpInfoResp, ResponseAccountState, TransactionInfoResp};
use futures::{channel::mpsc, select, stream::StreamExt};
use jsonrpc_pubsub::{
    typed::{Sink, Subscriber},
    SubscriptionId,
};
use std::time::Duration;
use zksync_storage::ConnectionPool;
use zksync_types::tx::TxHash;
use zksync_types::BlockNumber;
use zksync_types::{block::ExecutedOperations, ActionType, Address};

use self::{event_fetcher::EventFetcher, operation_notifier::OperationNotifier};

mod event_fetcher;
mod operation_notifier;
mod state;
mod sub_store;

const NOTIFIER_CHANNEL_CAPACITY: usize = 32_768;

#[derive(Debug)]
pub struct ExecutedOps {
    pub operations: Vec<ExecutedOperations>,
    pub block_number: BlockNumber,
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

#[derive(Debug)]
struct SubscriptionSender<T> {
    id: SubscriptionId,
    sink: Sink<T>,
}

pub fn start_sub_notifier(
    db_pool: ConnectionPool,
    mut subscription_stream: mpsc::Receiver<EventNotifierRequest>,
    api_requests_caches_size: usize,
    miniblock_interval: Duration,
) -> tokio::task::JoinHandle<()> {
    let (new_block_sender, mut new_block_receiver) = mpsc::channel(NOTIFIER_CHANNEL_CAPACITY);
    let (new_txs_sender, mut new_txs_receiver) = mpsc::channel(NOTIFIER_CHANNEL_CAPACITY);

    let mut notifier = OperationNotifier::new(api_requests_caches_size, db_pool.clone());

    tokio::spawn(async move {
        let fetcher = EventFetcher::new(
            db_pool,
            miniblock_interval,
            new_block_sender,
            new_txs_sender,
        )
        .await
        .expect("Unable to create event fetcher");

        tokio::spawn(fetcher.run());

        loop {
            select! {
                new_block = new_block_receiver.next() => {
                    if let Some(new_block) = new_block {
                        notifier.handle_new_block(new_block)
                            .await
                            .map_err(|e| log::warn!("Failed to handle new block: {}",e))
                            .unwrap_or_default();
                    }
                },
                new_exec_batch = new_txs_receiver.next() => {
                    if let Some(new_exec_batch) = new_exec_batch {
                        notifier.handle_new_executed_batch(new_exec_batch)
                            .map_err(|e| log::warn!("Failed to handle new exec batch: {}",e))
                            .unwrap_or_default();
                    }
                },
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
