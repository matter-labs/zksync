//! zkSync core private API server.
//!
//! This file contains endpoint expected to be used by
//! other components of zkSync stack **only**. This API must not be
//! available from outside of the cluster.
//!
//! All the incoming data is assumed to be correct and not double-checked
//! for correctness.

use crate::{eth_watch::EthWatchRequest, mempool::MempoolTransactionRequest};
use actix_web::{web, App, HttpResponse, HttpServer};
use futures::{
    channel::{mpsc, oneshot},
    sink::SinkExt,
};
use serde::Deserialize;
use std::{str::FromStr, thread};
use zksync_api_types::{
    v02::pagination::{ApiEither, PaginationDirection, PaginationQuery, PendingOpsRequest},
    PriorityOpLookupQuery,
};
use zksync_config::configs::api::PrivateApi;
use zksync_types::{tx::TxEthSignature, AccountId, Address, SignedZkSyncTx};
use zksync_utils::panic_notify::ThreadPanicNotify;

#[derive(Debug, Clone)]
struct AppState {
    mempool_tx_sender: mpsc::Sender<MempoolTransactionRequest>,
    eth_watch_req_sender: mpsc::Sender<EthWatchRequest>,
}

/// Adds a new transaction into the mempool.
/// Returns a JSON representation of `Result<(), TxAddError>`.
/// Expects transaction to be checked on the API side.
#[actix_web::post("/new_tx")]
async fn new_tx(
    data: web::Data<AppState>,
    web::Json(tx): web::Json<SignedZkSyncTx>,
) -> actix_web::Result<HttpResponse> {
    let (sender, receiver) = oneshot::channel();
    let item = MempoolTransactionRequest::NewTx(Box::new(tx), sender);
    let mut mempool_sender = data.mempool_tx_sender.clone();
    mempool_sender
        .send(item)
        .await
        .map_err(|_err| HttpResponse::InternalServerError().finish())?;

    let response = receiver
        .await
        .map_err(|_err| HttpResponse::InternalServerError().finish())?;

    Ok(HttpResponse::Ok().json(response))
}

/// Adds a new transactions batch into the mempool.
/// Returns a JSON representation of `Result<(), TxAddError>`.
/// Expects transaction to be checked on the API side.
#[actix_web::post("/new_txs_batch")]
async fn new_txs_batch(
    data: web::Data<AppState>,
    web::Json((txs, eth_signatures)): web::Json<(Vec<SignedZkSyncTx>, Vec<TxEthSignature>)>,
) -> actix_web::Result<HttpResponse> {
    let (sender, receiver) = oneshot::channel();
    let item = MempoolTransactionRequest::NewTxsBatch(txs, eth_signatures, sender);
    let mut mempool_sender = data.mempool_tx_sender.clone();
    mempool_sender
        .send(item)
        .await
        .map_err(|_err| HttpResponse::InternalServerError().finish())?;

    let response = receiver
        .await
        .map_err(|_err| HttpResponse::InternalServerError().finish())?;

    Ok(HttpResponse::Ok().json(response))
}

/// Obtains information about unconfirmed deposits known for a certain address.
#[actix_web::get("/unconfirmed_deposits/{address}")]
async fn unconfirmed_deposits(
    data: web::Data<AppState>,
    web::Path(address): web::Path<Address>,
) -> actix_web::Result<HttpResponse> {
    let (sender, receiver) = oneshot::channel();
    let item = EthWatchRequest::GetUnconfirmedDeposits {
        address,
        resp: sender,
    };
    let mut eth_watch_sender = data.eth_watch_req_sender.clone();
    eth_watch_sender
        .send(item)
        .await
        .map_err(|_err| HttpResponse::InternalServerError().finish())?;

    let response = receiver
        .await
        .map_err(|_err| HttpResponse::InternalServerError().finish())?;

    Ok(HttpResponse::Ok().json(response))
}

#[derive(Debug, Deserialize)]
struct PendingOpsFlattenRequest {
    pub address: Address,
    pub account_id: Option<AccountId>,
    pub serial_id: String,
    pub limit: u32,
    pub direction: PaginationDirection,
}

/// Obtains information about unconfirmed operations known for a certain account.
/// Pending deposits can be matched only with addresses,
/// while pending full exits can be matched only with account ids.
/// If the account isn't created yet it doesn't have an id
/// but we can still find pending deposits for its address that is why account_id is Option.
#[actix_web::get("/unconfirmed_ops")]
async fn unconfirmed_ops(
    data: web::Data<AppState>,
    web::Query(params): web::Query<PendingOpsFlattenRequest>,
) -> actix_web::Result<HttpResponse> {
    let (sender, receiver) = oneshot::channel();
    // Serializing enum query parameters doesn't work, so parse it separately.
    let serial_id = ApiEither::from_str(&params.serial_id)
        .map_err(|_| HttpResponse::InternalServerError().finish())?;
    let query = PaginationQuery {
        from: PendingOpsRequest {
            address: params.address,
            account_id: params.account_id,
            serial_id,
        },
        limit: params.limit,
        direction: params.direction,
    };
    let item = EthWatchRequest::GetUnconfirmedOps {
        query,
        resp: sender,
    };
    let mut eth_watch_sender = data.eth_watch_req_sender.clone();
    eth_watch_sender
        .send(item)
        .await
        .map_err(|_err| HttpResponse::InternalServerError().finish())?;

    let response = receiver
        .await
        .map_err(|_err| HttpResponse::InternalServerError().finish())?;

    Ok(HttpResponse::Ok().json(response))
}

/// Returns information about unconfirmed operation.
#[actix_web::post("/unconfirmed_op")]
async fn unconfirmed_op(
    data: web::Data<AppState>,
    web::Json(query): web::Json<PriorityOpLookupQuery>,
) -> actix_web::Result<HttpResponse> {
    let (sender, receiver) = oneshot::channel();
    let item = match query {
        PriorityOpLookupQuery::ByEthHash(eth_hash) => EthWatchRequest::GetUnconfirmedOpByEthHash {
            eth_hash,
            resp: sender,
        },
        PriorityOpLookupQuery::BySyncHash(tx_hash) => EthWatchRequest::GetUnconfirmedOpByTxHash {
            tx_hash,
            resp: sender,
        },
        PriorityOpLookupQuery::ByAnyHash(hash) => {
            EthWatchRequest::GetUnconfirmedOpByAnyHash { hash, resp: sender }
        }
    };
    let mut eth_watch_sender = data.eth_watch_req_sender.clone();
    eth_watch_sender
        .send(item)
        .await
        .map_err(|_err| HttpResponse::InternalServerError().finish())?;

    let response = receiver
        .await
        .map_err(|_err| HttpResponse::InternalServerError().finish())?;

    Ok(HttpResponse::Ok().json(response))
}

#[allow(clippy::too_many_arguments)]
pub fn start_private_core_api(
    panic_notify: mpsc::Sender<bool>,
    mempool_tx_sender: mpsc::Sender<MempoolTransactionRequest>,
    eth_watch_req_sender: mpsc::Sender<EthWatchRequest>,
    config: PrivateApi,
) {
    thread::Builder::new()
        .name("core-private-api".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());
            let mut actix_runtime = actix_rt::System::new("core-private-api-server");

            actix_runtime.block_on(async move {
                // Start HTTP server.
                HttpServer::new(move || {
                    let app_state = AppState {
                        mempool_tx_sender: mempool_tx_sender.clone(),
                        eth_watch_req_sender: eth_watch_req_sender.clone(),
                    };

                    // By calling `register_data` instead of `data` we're avoiding double
                    // `Arc` wrapping of the object.
                    App::new()
                        .wrap(actix_web::middleware::Logger::default())
                        .wrap(vlog::actix_middleware())
                        .app_data(web::Data::new(app_state))
                        .app_data(web::JsonConfig::default().limit(2usize.pow(32)))
                        .service(new_tx)
                        .service(new_txs_batch)
                        .service(unconfirmed_op)
                        .service(unconfirmed_ops)
                        .service(unconfirmed_deposits)
                })
                .bind(&config.bind_addr())
                .expect("failed to bind")
                .run()
                .await
            })
        })
        .expect("failed to start prover server");
}
