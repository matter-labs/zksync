#![allow(clippy::needless_return)]

// Built-in deps
use std::sync::Arc;
// External uses
use futures::channel::mpsc;
use jsonrpc_core::{MetaIoHandler, Result};
use jsonrpc_derive::rpc;
use jsonrpc_pubsub::{typed::Subscriber, PubSubHandler, Session, SubscriptionId};
use jsonrpc_ws_server::RequestContext;
// Workspace uses
use zksync_config::ConfigurationOptions;
use zksync_storage::ConnectionPool;
use zksync_types::{tx::TxHash, ActionType, Address};
// Local uses
use crate::fee_ticker::TickerRequest;
use crate::{
    api_server::event_notify::{start_sub_notifier, EventNotifierRequest, EventSubscribeRequest},
    api_server::rpc_server::types::{ETHOpInfoResp, ResponseAccountState, TransactionInfoResp},
    signature_checker::VerifyTxSignatureRequest,
};
use zksync_utils::panic_notify::ThreadPanicNotify;

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
pub fn start_ws_server(
    config_options: &ConfigurationOptions,
    db_pool: ConnectionPool,
    sign_verify_request_sender: mpsc::Sender<VerifyTxSignatureRequest>,
    ticker_request_sender: mpsc::Sender<TickerRequest>,
    panic_notify: mpsc::Sender<bool>,
) {
    let config_options = config_options.clone();
    let api_caches_size = config_options.api_requests_caches_size;

    let addr = config_options.json_rpc_ws_server_address;

    let (event_sub_sender, event_sub_receiver) = mpsc::channel(2048);

    start_sub_notifier(
        db_pool.clone(),
        event_sub_receiver,
        api_caches_size,
        config_options
            .miniblock_timings
            .miniblock_iteration_interval,
    );

    let req_rpc_app = super::rpc_server::RpcApp::new(
        &config_options,
        db_pool,
        sign_verify_request_sender,
        ticker_request_sender,
    );

    std::thread::spawn(move || {
        let _panic_sentinel = ThreadPanicNotify(panic_notify);

        let mut io = PubSubHandler::new(MetaIoHandler::default());

        req_rpc_app.extend(&mut io);

        let rpc_sub_app = RpcSubApp { event_sub_sender };

        io.extend_with(rpc_sub_app.to_delegate());

        let task_executor = tokio_old::runtime::Builder::new()
            .name_prefix("ws-executor")
            .core_threads(super::THREADS_PER_SERVER)
            .build()
            .expect("failed to build ws executor");

        let server = jsonrpc_ws_server::ServerBuilder::with_meta_extractor(
            io,
            |context: &RequestContext| Arc::new(Session::new(context.sender())),
        )
        .request_middleware(super::loggers::ws_rpc::request_middleware)
        .max_connections(1000)
        .event_loop_executor(task_executor.executor())
        .start(&addr)
        .expect("Unable to start RPC ws server");

        server.wait().expect("rpc ws server start");
    });
}
