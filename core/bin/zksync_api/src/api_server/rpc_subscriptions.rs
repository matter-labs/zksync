#![allow(clippy::needless_return)]

// Built-in deps
use std::sync::Arc;
use std::time::Duration;
// External uses
use futures::channel::mpsc;
use jsonrpc_core::{MetaIoHandler, Result};
use jsonrpc_derive::rpc;
use jsonrpc_pubsub::{typed::Subscriber, PubSubHandler, Session, SubscriptionId};
use jsonrpc_ws_server::RequestContext;
use tokio::task::JoinHandle;
// Workspace uses
use zksync_config::configs::api::{CommonApiConfig, JsonRpcConfig, TokenConfig};
use zksync_mempool::MempoolTransactionRequest;
use zksync_storage::ConnectionPool;
use zksync_types::{tx::TxHash, ActionType, Address, ChainId};
use zksync_utils::panic_notify::{spawn_panic_handler, ThreadPanicNotify};
// Local uses
use crate::fee_ticker::FeeTicker;
use crate::{
    api_server::event_notify::{start_sub_notifier, EventNotifierRequest, EventSubscribeRequest},
    api_server::rpc_server::types::{ETHOpInfoResp, ResponseAccountState, TransactionInfoResp},
    signature_checker::VerifySignatureRequest,
};

#[rpc]
pub trait RpcPubSub {
    type Metadata;

    #[pubsub(subscription = "tx", subscribe, name = "tx_subscribe", alias("tx_sub"))]
    fn subscribe_tx(
        &self,
        meta: Self::Metadata,
        subscriber: Subscriber<TransactionInfoResp>,
        hash: TxHash,
        action_type: ActionType,
    );
    #[pubsub(subscription = "tx", unsubscribe, name = "tx_unsubscribe")]
    fn unsubscribe_tx(
        &self,
        meta: Option<Self::Metadata>,
        subscription: SubscriptionId,
    ) -> Result<bool>;

    #[pubsub(
        subscription = "eth_op",
        subscribe,
        name = "ethop_subscribe",
        alias("ethop_sub")
    )]
    fn subscribe_ethop(
        &self,
        meta: Self::Metadata,
        subscriber: Subscriber<ETHOpInfoResp>,
        serial_id: u64,
        action_type: ActionType,
    );
    #[pubsub(subscription = "eth_op", unsubscribe, name = "ethop_unsubscribe")]
    fn unsubscribe_ethop(
        &self,
        meta: Option<Self::Metadata>,
        subscription: SubscriptionId,
    ) -> Result<bool>;

    #[pubsub(
        subscription = "account",
        subscribe,
        name = "account_subscribe",
        alias("account_sub")
    )]
    fn subscribe_account(
        &self,
        meta: Self::Metadata,
        subscriber: Subscriber<ResponseAccountState>,
        addr: Address,
        action_type: ActionType,
    );
    #[pubsub(subscription = "account", unsubscribe, name = "account_unsubscribe")]
    fn unsubscribe_account(
        &self,
        meta: Option<Self::Metadata>,
        subscription: SubscriptionId,
    ) -> Result<bool>;
}

impl RpcPubSub for RpcSubApp {
    type Metadata = Arc<Session>;

    // subscribe - sub id, sink
    // unsub - sub id

    fn subscribe_tx(
        &self,
        _meta: Self::Metadata,
        subscriber: Subscriber<TransactionInfoResp>,
        hash: TxHash,
        action: ActionType,
    ) {
        self.event_sub_sender
            .clone()
            .try_send(EventNotifierRequest::Sub(
                EventSubscribeRequest::Transaction {
                    hash,
                    action,
                    subscriber,
                },
            ))
            .unwrap_or_default();
    }
    fn unsubscribe_tx(&self, _meta: Option<Self::Metadata>, id: SubscriptionId) -> Result<bool> {
        self.event_sub_sender
            .clone()
            .try_send(EventNotifierRequest::Unsub(id))
            .unwrap_or_default();
        Ok(true)
    }

    fn subscribe_ethop(
        &self,
        _meta: Self::Metadata,
        subscriber: Subscriber<ETHOpInfoResp>,
        serial_id: u64,
        action: ActionType,
    ) {
        self.event_sub_sender
            .clone()
            .try_send(EventNotifierRequest::Sub(
                EventSubscribeRequest::PriorityOp {
                    serial_id,
                    action,
                    subscriber,
                },
            ))
            .unwrap_or_default();
    }
    fn unsubscribe_ethop(&self, _meta: Option<Self::Metadata>, id: SubscriptionId) -> Result<bool> {
        self.event_sub_sender
            .clone()
            .try_send(EventNotifierRequest::Unsub(id))
            .unwrap_or_default();
        Ok(true)
    }

    fn subscribe_account(
        &self,
        _meta: Self::Metadata,
        subscriber: Subscriber<ResponseAccountState>,
        address: Address,
        action: ActionType,
    ) {
        self.event_sub_sender
            .clone()
            .try_send(EventNotifierRequest::Sub(EventSubscribeRequest::Account {
                address,
                action,
                subscriber,
            }))
            .unwrap_or_default();
    }

    fn unsubscribe_account(
        &self,
        _meta: Option<Self::Metadata>,
        id: SubscriptionId,
    ) -> Result<bool> {
        self.event_sub_sender
            .clone()
            .try_send(EventNotifierRequest::Unsub(id))
            .unwrap_or_default();
        Ok(true)
    }
}

struct RpcSubApp {
    event_sub_sender: mpsc::Sender<EventNotifierRequest>,
}

#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn start_ws_server(
    db_pool: ConnectionPool,
    sign_verify_request_sender: mpsc::Sender<VerifySignatureRequest>,
    ticker: FeeTicker,
    common_config: &CommonApiConfig,
    token_config: &TokenConfig,
    config: &JsonRpcConfig,
    miniblock_iteration_interval: Duration,
    mempool_tx_sender: mpsc::Sender<MempoolTransactionRequest>,
    confirmations_for_eth_event: u64,
    chain_id: ChainId,
) -> JoinHandle<()> {
    let addr = config.ws_bind_addr();

    let (event_sub_sender, event_sub_receiver) = mpsc::channel(2048);

    start_sub_notifier(
        db_pool.clone(),
        event_sub_receiver,
        common_config.caches_size,
        miniblock_iteration_interval,
        token_config,
    );

    let req_rpc_app = super::rpc_server::RpcApp::new(
        db_pool,
        sign_verify_request_sender,
        ticker,
        common_config,
        token_config,
        confirmations_for_eth_event,
        chain_id,
        mempool_tx_sender,
    );

    let (handler, panic_sender) = spawn_panic_handler();

    std::thread::spawn(move || {
        let _panic_sentinel = ThreadPanicNotify(panic_sender);
        let mut io = PubSubHandler::new(MetaIoHandler::default());

        req_rpc_app.extend(&mut io);

        let rpc_sub_app = RpcSubApp { event_sub_sender };

        io.extend_with(rpc_sub_app.to_delegate());

        let server = jsonrpc_ws_server::ServerBuilder::with_meta_extractor(
            io,
            |context: &RequestContext| Arc::new(Session::new(context.sender())),
        )
        .max_connections(1000)
        .start(&addr)
        .expect("Unable to start RPC ws server");

        server.wait().expect("rpc ws server start");
    });
    handler
}
