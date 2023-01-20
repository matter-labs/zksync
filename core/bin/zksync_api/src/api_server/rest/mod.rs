use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer};
use futures::channel::mpsc;
use std::net::SocketAddr;
use zksync_storage::ConnectionPool;
use zksync_types::{ChainId, SequentialTxId, H160};

use zksync_utils::panic_notify::{spawn_panic_handler, ThreadPanicNotify};

use self::v01::api_decl::ApiV01;
use crate::signature_checker::VerifySignatureRequest;

use super::tx_sender::TxSender;

use crate::api_server::rest::network_status::SharedNetworkStatus;
use crate::fee_ticker::FeeTicker;
use tokio::task::JoinHandle;
use zksync_config::ZkSyncConfig;
use zksync_mempool::MempoolTransactionRequest;

mod forced_exit_requests;
mod helpers;
pub mod network_status;
mod v01;
pub mod v02;

async fn start_server(
    api_v01: ApiV01,
    fee_ticker: FeeTicker,
    sign_verifier: mpsc::Sender<VerifySignatureRequest>,
    bind_to: SocketAddr,
    mempool_tx_sender: mpsc::Sender<MempoolTransactionRequest>,
    chain_id: ChainId,
) {
    HttpServer::new(move || {
        let api_v01 = api_v01.clone();
        // This api stores forced exit requests, it's necessary to use main database connection
        let forced_exit_requests_api_scope = forced_exit_requests::api_scope(
            api_v01.main_database_connection_pool.clone(),
            api_v01
                .config
                .api
                .common
                .forced_exit_minimum_account_age_secs,
            &api_v01.config.forced_exit_requests,
            api_v01.config.contracts.forced_exit_addr,
        );

        let api_v02_scope = {
            let tx_sender = TxSender::new(
                api_v01.connection_pool.clone(),
                sign_verifier.clone(),
                fee_ticker.clone(),
                &api_v01.config.api.common,
                &api_v01.config.api.token_config,
                mempool_tx_sender.clone(),
                chain_id,
            );
            v02::api_scope(tx_sender, &api_v01.config, api_v01.network_status.clone())
        };
        App::new()
            .wrap(
                Cors::default()
                    .send_wildcard()
                    .max_age(3600)
                    .allow_any_origin()
                    .allow_any_header()
                    .allow_any_method(),
            )
            .service(api_v01.into_scope())
            .service(forced_exit_requests_api_scope)
            .service(api_v02_scope)
            // Endpoint needed for js isReachable
            .route(
                "/favicon.ico",
                web::get().to(|| HttpResponse::Ok().finish()),
            )
    })
    .workers(super::THREADS_PER_SERVER)
    .bind(bind_to)
    .unwrap()
    .shutdown_timeout(60)
    .keep_alive(10)
    .client_timeout(60000)
    .run()
    .await
    .expect("REST API server has crashed");
}

/// Start HTTP REST API
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn start_server_thread_detached(
    read_only_connection_pool: ConnectionPool,
    main_database_connection_pool: ConnectionPool,
    listen_addr: SocketAddr,
    contract_address: H160,
    fee_ticker: FeeTicker,
    sign_verifier: mpsc::Sender<VerifySignatureRequest>,
    chain_id: ChainId,
    mempool_tx_sender: mpsc::Sender<MempoolTransactionRequest>,
    core_address: String,
) -> JoinHandle<()> {
    let (handler, panic_sender) = spawn_panic_handler();

    std::thread::Builder::new()
        .name("actix-rest-api".to_string())
        .spawn(move || {
            actix_rt::System::new().block_on(async move {
                let _panic_sentinel = ThreadPanicNotify(panic_sender.clone());
                // TODO remove this config ZKS-815
                let config = ZkSyncConfig::from_env();

                let mut network_status = SharedNetworkStatus::new(core_address);
                // We want to update the network status, as soon as possible, otherwise we can catch the situation,
                // when the node is started and receiving the request, but the status is still `null` and
                // monitoring tools spawn the notification that our node is down, though it's just a default status
                // We want to run the first query inside the main replica because there is no load on the main replica
                // and this will distribute the load between the nodes. As another benefit, it won't mess up the cache
                // inside the replica

                let last_tx_id = network_status
                    .update(&main_database_connection_pool, SequentialTxId(0))
                    .await
                    .unwrap();

                let api_v01 = ApiV01::new(
                    read_only_connection_pool,
                    main_database_connection_pool,
                    contract_address,
                    config,
                    network_status,
                );

                api_v01.spawn_network_status_updater(panic_sender, last_tx_id);

                start_server(
                    api_v01,
                    fee_ticker,
                    sign_verifier,
                    listen_addr,
                    mempool_tx_sender.clone(),
                    chain_id,
                )
                .await;
            });
        })
        .expect("Api server thread");
    handler
}
