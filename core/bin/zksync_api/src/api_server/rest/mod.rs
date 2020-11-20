use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use futures::channel::mpsc;
use std::net::SocketAddr;
use zksync_config::{ApiServerOptions, ConfigurationOptions};
use zksync_storage::ConnectionPool;
use zksync_types::H160;

use zksync_utils::panic_notify::ThreadPanicNotify;

use self::v01::api_decl::ApiV01;
use crate::{fee_ticker::TickerRequest, signature_checker::VerifyTxSignatureRequest};

use super::tx_sender::TxSender;

mod helpers;
mod v01;
pub mod v1;

async fn start_server(
    api_v01: ApiV01,
    fee_ticker: mpsc::Sender<TickerRequest>,
    sign_verifier: mpsc::Sender<VerifyTxSignatureRequest>,
    bind_to: SocketAddr,
) {
    let logger_format = crate::api_server::loggers::rest::get_logger_format();

    HttpServer::new(move || {
        let api_v01 = api_v01.clone();

        let api_v1_scope = {
            let env_options = api_v01.config_options.clone();
            let api_server_options = api_v01.api_server_options.clone();

            let tx_sender = TxSender::new(
                api_v01.connection_pool.clone(),
                sign_verifier.clone(),
                fee_ticker.clone(),
                &api_server_options,
            );
            v1::api_scope(tx_sender, env_options, api_server_options)
        };

        App::new()
            .wrap(middleware::Logger::new(&logger_format))
            .wrap(Cors::new().send_wildcard().max_age(3600).finish())
            .service(api_v01.into_scope())
            .service(api_v1_scope)
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
pub(super) fn start_server_thread_detached(
    connection_pool: ConnectionPool,
    listen_addr: SocketAddr,
    contract_address: H160,
    panic_notify: mpsc::Sender<bool>,
    fee_ticker: mpsc::Sender<TickerRequest>,
    sign_verifier: mpsc::Sender<VerifyTxSignatureRequest>,
    config_options: ConfigurationOptions,
    api_server_options: ApiServerOptions,
) {
    std::thread::Builder::new()
        .name("actix-rest-api".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());

            actix_rt::System::new("api-server").block_on(async move {
                let api_v01 = ApiV01::new(
                    connection_pool,
                    contract_address,
                    config_options,
                    api_server_options,
                );
                api_v01.spawn_network_status_updater(panic_notify);

                start_server(api_v01, fee_ticker, sign_verifier, listen_addr).await;
            });
        })
        .expect("Api server thread");
}
