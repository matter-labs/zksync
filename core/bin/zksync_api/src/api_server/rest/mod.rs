use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use futures::channel::mpsc;
use std::net::SocketAddr;
use zksync_config::ConfigurationOptions;
use zksync_storage::ConnectionPool;
use zksync_types::H160;

use zksync_utils::panic_notify::ThreadPanicNotify;

use self::v01::api_decl::ApiV01;

mod helpers;
mod v01;

async fn start_server(api_v01: ApiV01, bind_to: SocketAddr) {
    let logger_format = crate::api_server::loggers::rest::get_logger_format();
    HttpServer::new(move || {
        let api_v01 = api_v01.clone();
        App::new()
            .wrap(middleware::Logger::new(&logger_format))
            .wrap(Cors::new().send_wildcard().max_age(3600).finish())
            .service(api_v01.into_scope())
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
pub(super) fn start_server_thread_detached(
    connection_pool: ConnectionPool,
    listen_addr: SocketAddr,
    contract_address: H160,
    panic_notify: mpsc::Sender<bool>,
    config_options: ConfigurationOptions,
) {
    std::thread::Builder::new()
        .name("actix-rest-api".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());

            actix_rt::System::new("api-server").block_on(async move {
                let api_v01 = ApiV01::new(connection_pool, contract_address, config_options);
                api_v01.spawn_network_status_updater(panic_notify);

                start_server(api_v01, listen_addr).await;
            });
        })
        .expect("Api server thread");
}
