//! zkSync core private API server.
//!
//! This file contains endpoint expected to be used by
//! other components of zkSync stack **only**. This API must not be
//! available from outside of the cluster.
//!
//! All the incoming data is assumed to be correct and not double-checked
//! for correctness.

use std::thread;

use actix_web::{web, App, HttpResponse, HttpServer};
use futures::{channel::mpsc, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use zksync_config::configs::api::PrivateApiConfig;
use zksync_eth_client::EthereumGateway;
use zksync_storage::ConnectionPool;
use zksync_utils::panic_notify::ThreadPanicNotify;

#[derive(Debug, Clone)]
struct AppState {
    connection_pool: ConnectionPool,
    read_only_connection_pool: ConnectionPool,
    eth_client: EthereumGateway,
}

#[derive(Serialize, Deserialize)]
struct CoreStatus {
    main_database_status: bool,
    replica_database_status: bool,
    eth_status: bool,
}

/// Health check.
/// The core actor should have a connection to main and replica database and have connection to eth
#[actix_web::get("/status")]
async fn status(data: web::Data<AppState>) -> actix_web::Result<HttpResponse> {
    let main_database_connection_status = data.connection_pool.access_storage().await.is_ok();
    let replica_database_connection_status = data
        .read_only_connection_pool
        .access_storage()
        .await
        .is_ok();
    let eth_status = data.eth_client.block_number().await.is_ok();

    let response = CoreStatus {
        main_database_status: main_database_connection_status,
        replica_database_status: replica_database_connection_status,
        eth_status,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub fn start_private_core_api(
    connection_pool: ConnectionPool,
    read_only_connection_pool: ConnectionPool,
    eth_client: EthereumGateway,
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
                        connection_pool: connection_pool.clone(),
                        read_only_connection_pool: read_only_connection_pool.clone(),
                        eth_client: eth_client.clone(),
                    };

                    // By calling `register_data` instead of `data` we're avoiding double
                    // `Arc` wrapping of the object.
                    App::new()
                        .wrap(actix_web::middleware::Logger::default())
                        .app_data(web::Data::new(app_state))
                        .app_data(web::JsonConfig::default().limit(2usize.pow(32)))
                        .service(status)
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
