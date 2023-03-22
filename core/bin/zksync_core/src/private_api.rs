//! zkSync core private API server.
//!
//! This file contains endpoint expected to be used by
//! other components of zkSync stack **only**. This API must not be
//! available from outside of the cluster.
//!
//! All the incoming data is assumed to be correct and not double-checked
//! for correctness.

use std::thread;
use std::time::{Duration, Instant};

use actix_web::{web, App, HttpResponse, HttpServer};
use futures::{channel::mpsc, StreamExt};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use zksync_api_types::CoreStatus;

use zksync_config::configs::api::PrivateApiConfig;
use zksync_eth_client::EthereumGateway;
use zksync_storage::ConnectionPool;
use zksync_utils::panic_notify::ThreadPanicNotify;

const STATUS_INVALIDATION_PERIOD: Duration = Duration::from_secs(60);

#[derive(Debug)]
struct AppState {
    connection_pool: ConnectionPool,
    read_only_connection_pool: ConnectionPool,
    eth_client: EthereumGateway,
    status_cache: RwLock<Option<(CoreStatus, Instant)>>,
}

/// Health check.
/// The core actor is expected have connection to web3 and both main/replica databases
#[actix_web::get("/status")]
async fn status(data: web::Data<AppState>) -> actix_web::Result<HttpResponse> {
    if let Some((status, data)) = data.status_cache.read().await.as_ref() {
        if data.elapsed() < STATUS_INVALIDATION_PERIOD {
            return Ok(HttpResponse::Ok().json(status.clone()));
        }
    }

    // We need to get a lock here so we don't abuse the database and eth node connections
    // with multiple requests from other API nodes when the cache has been invalidated.

    let mut status = data.status_cache.write().await;
    let main_database_status = data.connection_pool.access_storage().await.is_ok();
    let replica_database_status = data
        .read_only_connection_pool
        .access_storage()
        .await
        .is_ok();
    let eth_status = data.eth_client.block_number().await.is_ok();

    let response = CoreStatus {
        main_database_available: main_database_status,
        replica_database_available: replica_database_status,
        web3_available: eth_status,
    };
    *status = Some((response.clone(), Instant::now()));

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
                        status_cache: Default::default(),
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
                .workers(1)
                .run()
                .await
            })
        })
        .expect("failed to start prover server");
    tokio::spawn(async move {
        panic_receiver.next().await.unwrap();
    })
}
