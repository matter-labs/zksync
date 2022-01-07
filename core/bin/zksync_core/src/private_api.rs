//! zkSync core private API server.
//!
//! This file contains endpoint expected to be used by
//! other components of zkSync stack **only**. This API must not be
//! available from outside of the cluster.
//!
//! All the incoming data is assumed to be correct and not double-checked
//! for correctness.

use crate::mempool::MempoolTransactionRequest;
use actix_web::error::InternalError;
use actix_web::{web, App, HttpResponse, HttpServer};
use futures::{
    channel::{mpsc, oneshot},
    sink::SinkExt,
    StreamExt,
};

use std::thread;
use tokio::task::JoinHandle;
use zksync_config::configs::api::PrivateApiConfig;
use zksync_types::{tx::TxEthSignature, SignedZkSyncTx};
use zksync_utils::panic_notify::ThreadPanicNotify;

#[derive(Debug, Clone)]
struct AppState {
    mempool_tx_sender: mpsc::Sender<MempoolTransactionRequest>,
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
    mempool_sender.send(item).await.map_err(|err| {
        InternalError::from_response(err, HttpResponse::InternalServerError().finish())
    })?;

    let response = receiver.await.map_err(|err| {
        InternalError::from_response(err, HttpResponse::InternalServerError().finish())
    })?;

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
    mempool_sender.send(item).await.map_err(|err| {
        InternalError::from_response(err, HttpResponse::InternalServerError().finish())
    })?;

    let response = receiver.await.map_err(|err| {
        InternalError::from_response(err, HttpResponse::InternalServerError().finish())
    })?;

    Ok(HttpResponse::Ok().json(response))
}

#[allow(clippy::too_many_arguments)]
pub fn start_private_core_api(
    mempool_tx_sender: mpsc::Sender<MempoolTransactionRequest>,
    config: PrivateApiConfig,
) -> JoinHandle<()> {
    let (panic_sender, mut panic_receiver) = mpsc::channel(1);

    thread::Builder::new()
        .name("core-private-api".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_sender.clone());
            let actix_runtime = actix_rt::System::new();

            actix_runtime.block_on(async move {
                // Start HTTP server.
                HttpServer::new(move || {
                    let app_state = AppState {
                        mempool_tx_sender: mempool_tx_sender.clone(),
                    };

                    // By calling `register_data` instead of `data` we're avoiding double
                    // `Arc` wrapping of the object.
                    App::new()
                        .wrap(actix_web::middleware::Logger::default())
                        .app_data(web::Data::new(app_state))
                        .app_data(web::JsonConfig::default().limit(2usize.pow(32)))
                        .service(new_tx)
                        .service(new_txs_batch)
                })
                .bind(&config.bind_addr())
                .expect("failed to bind")
                .run()
                .await
            })
        })
        .expect("failed to start prover server");
    tokio::spawn(async move {
        panic_receiver.next().await.unwrap();
    })
}
