use crate::ThreadPanicNotify;
use futures::Future;
use jsonrpc_core::{IoHandler, MetaIoHandler, BoxFuture, Params};
use jsonrpc_core::{Error, Result, ErrorCode};
use jsonrpc_derive::rpc;
use jsonrpc_http_server::ServerBuilder;
use models::node::{Account, AccountAddress, AccountId, FranklinTx};
use std::sync::{mpsc, Arc, Mutex};
use futures::sync::mpsc as fmpsc;
use storage::{ConnectionPool, StorageProcessor, Token, TxAddError};

use jsonrpc_pubsub::{PubSubHandler, Session, typed::Subscriber, SubscriptionId};
use jsonrpc_ws_server::RequestContext;
use serde_json::Value;
use models::StateKeeperRequest;
use super::event_notify::EventSubscribe;
use rand::Rng;

#[rpc]
pub trait RpcPubSub {
    type Metadata;

    /// Hello subscription
    #[pubsub(subscription = "hello", subscribe, name = "hello_subscribe", alias("hello_sub"))]
    fn subscribe(&self, meta: Self::Metadata, subscriber: Subscriber<Account>, param: AccountAddress);

    /// Unsubscribe from hello subscription.
    #[pubsub(subscription = "hello", unsubscribe, name = "hello_unsubscribe")]
    fn unsubscribe(&self, meta: Option<Self::Metadata>, subscription: SubscriptionId) -> Result<bool>;
}

impl RpcSubApp {
}

impl RpcPubSub for RpcSubApp {
    type Metadata = Arc<Session>;


    // subscribe - sub id, sink
    // unsub - sub id

    fn subscribe(&self, _meta: Self::Metadata, subscriber: Subscriber<Acount>, param: AccountAddress) {
        let sub_id = rand::thread_rng().gen::<u64>();
//            if param != 10 {
//                subscriber
//                    .reject(Error {
//                        code: ErrorCode::InvalidParams,
//                        message: "Rejecting subscription - invalid parameters provided.".into(),
//                        data: None,
//                    })
//                    .unwrap();
//                return;
//            }



//        let id = self.uid.fetch_add(1, atomic::Ordering::SeqCst);
//        let sub_id = SubscriptionId::Number(id as u64);
//        let sink = subscriber.assign_id(sub_id.clone()).unwrap();
//        self.active.write().unwrap().insert(sub_id, sink);
    }

    fn unsubscribe(&self, _meta: Option<Self::Metadata>, id: SubscriptionId) -> Result<bool> {
        Ok(true)
//        let removed = self.active.write().unwrap().remove(&id);
//        if removed.is_some() {
//            Ok(true)
//        } else {
//            Err(Error {
//                code: ErrorCode::InvalidParams,
//                message: "Invalid subscription.".into(),
//                data: None,
//            })
//        }
    }
}

struct RpcSubApp {
    event_sub_req: fmpsc::Sender<EventSubscribe>,
}

fn start_ws_server() {
//    let (event_sub_sender, event_sub_receiver) = fmpsc::channel(2048);

    let mut io = PubSubHandler::new(MetaIoHandler::default());

    let rpc_sub_app = RpcSubApp;

    io.extend_with(rpc_sub_app.to_delegate());

    std::thread::Builder::new()
        .name("json_rpc_ws".to_string()).spawn(move || {
        let server =
            jsonrpc_ws_server::ServerBuilder::with_meta_extractor(io, |context: &RequestContext| std::sync::Arc::new(Session::new(context.sender())))
                .start(&"127.0.0.1:3031".parse().unwrap())
                .expect("Unable to start RPC ws server");

        server.wait();
    }).expect("JSON RPC ws thread");
}

