use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer};
use futures::channel::mpsc;
use std::net::SocketAddr;
use zksync_storage::ConnectionPool;
use zksync_types::H160;

use zksync_utils::panic_notify::{spawn_panic_handler, ThreadPanicNotify};

use self::v01::api_decl::ApiV01;
use crate::signature_checker::VerifySignatureRequest;

use super::tx_sender::TxSender;

use crate::fee_ticker::FeeTicker;
use tokio::task::JoinHandle;
use zksync_config::ZkSyncConfig;

mod forced_exit_requests;
mod helpers;
mod v01;
pub mod v02;

async fn start_server(
    api_v01: ApiV01,
    fee_ticker: FeeTicker,
    sign_verifier: mpsc::Sender<VerifySignatureRequest>,
    bind_to: SocketAddr,
) {
    HttpServer::new(move || {
        let api_v01 = api_v01.clone();

        let forced_exit_requests_api_scope = forced_exit_requests::api_scope(
            api_v01.connection_pool.clone(),
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
                api_v01.config.api.private.url.clone(),
            );
            v02::api_scope(tx_sender, &api_v01.config)
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
    .shutdown_timeout(1)
    .run()
    .await
    .expect("REST API server has crashed");
}

/// Start HTTP REST API
#[allow(clippy::too_many_arguments)]
pub fn start_server_thread_detached(
    connection_pool: ConnectionPool,
    listen_addr: SocketAddr,
    contract_address: H160,
    fee_ticker: FeeTicker,
    sign_verifier: mpsc::Sender<VerifySignatureRequest>,
    private_url: String,
) -> JoinHandle<()> {
    let (handler, panic_sender) = spawn_panic_handler();

    std::thread::Builder::new()
        .name("actix-rest-api".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_sender.clone());

            actix_rt::System::new().block_on(async move {
                // TODO remove this config ZKS-815
                let config = ZkSyncConfig::from_env();

                let api_v01 = ApiV01::new(connection_pool, contract_address, private_url, config);
                api_v01.spawn_network_status_updater(panic_sender);

                start_server(api_v01, fee_ticker, sign_verifier, listen_addr).await;
            });
        })
        .expect("Api server thread");
    handler
}
