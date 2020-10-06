//! zkSync core private API server.
//!
//! This file contains endpoint expected to be used by
//! other components of zkSync stack **only**. This API must not be
//! available from outside of the cluster.
//!
//! All the incoming data is assumed to be correct and not double-checked
//! for correctness.

use crate::mempool::MempoolRequest;
use actix_web::{web, App, HttpResponse, HttpServer};
use futures::{
    channel::{mpsc, oneshot},
    sink::SinkExt,
};
use std::thread;
use zksync_config::ConfigurationOptions;
use zksync_types::SignedFranklinTx;
use zksync_utils::panic_notify::ThreadPanicNotify;

#[derive(Debug, Clone)]
struct AppState {
    mempool_tx_sender: mpsc::Sender<MempoolRequest>,
}

/// Adds a new transaction into the mempool.
/// Returns a JSON representation of `Result<(), TxAddError>`.
/// Expects transaction to be checked on the API side.
async fn new_tx(
    data: web::Data<AppState>,
    web::Json(tx): web::Json<SignedFranklinTx>,
) -> actix_web::Result<HttpResponse> {
    let (sender, receiver) = oneshot::channel();
    let item = MempoolRequest::NewTx(Box::new(tx), sender);
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
async fn new_txs_batch(
    data: web::Data<AppState>,
    web::Json(txs): web::Json<Vec<SignedFranklinTx>>,
) -> actix_web::Result<HttpResponse> {
    let (sender, receiver) = oneshot::channel();
    let item = MempoolRequest::NewTxsBatch(txs, sender);
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

#[allow(clippy::too_many_arguments)]
pub fn start_private_core_api(
    config_options: ConfigurationOptions,
    panic_notify: mpsc::Sender<bool>,
    mempool_tx_sender: mpsc::Sender<MempoolRequest>,
) {
    thread::Builder::new()
        .name("prover_server".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());
            let mut actix_runtime = actix_rt::System::new("prover-server");

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
                        .route("/new_tx", web::post().to(new_tx))
                        .route("/new_txs_batch", web::post().to(new_txs_batch))
                })
                .bind(&config_options.core_server_address)
                .expect("failed to bind")
                .run()
                .await
            })
        })
        .expect("failed to start prover server");
}
