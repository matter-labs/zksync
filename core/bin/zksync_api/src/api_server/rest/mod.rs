use actix_cors::Cors;
use actix_web::{
    dev::{Body, ResponseBody, Service, ServiceRequest, ServiceResponse, Transform},
    web, App, Error, HttpResponse, HttpServer,
};
use futures::channel::mpsc;
use std::net::SocketAddr;
use zksync_storage::ConnectionPool;
use zksync_types::H160;

use zksync_utils::panic_notify::ThreadPanicNotify;

use self::v01::api_decl::ApiV01;
use crate::{fee_ticker::TickerRequest, signature_checker::VerifySignatureRequest};

use super::tx_sender::TxSender;
use futures::future::{ok, Ready};
use futures::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use zksync_config::ZkSyncConfig;

mod helpers;
mod v01;
pub mod v02;
pub mod v1;

pub struct SayHi;

// Middleware factory is `Transform` trait from actix-service crate
// `S` - type of the next service
// `B` - type of response's body
impl<S, B> Transform<S> for SayHi
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = SayHiMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(SayHiMiddleware { service })
    }
}

pub struct SayHiMiddleware<S> {
    service: S,
}

impl<S, B> Service for SayHiMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        println!("Hi from start. You requested: {}", req.path());

        let fut = self.service.call(req);

        Box::pin(async move {
            let mut res = fut.await?;
            let body = res.take_body();
            match body {
                ResponseBody::Other(b) => {
                    match b {
                        Body::Message(_message) => {
                            //
                        }
                        _ => {
                            panic!("not message");
                        }
                    }
                }
                _ => {
                    panic!("not other");
                }
            };

            let new_res = ServiceResponse::new(
                res.request().clone(),
                HttpResponse::Ok().body("fredbob").into_body(),
            );

            Ok(new_res)
        })
    }
}

async fn start_server(
    api_v01: ApiV01,
    fee_ticker: mpsc::Sender<TickerRequest>,
    sign_verifier: mpsc::Sender<VerifySignatureRequest>,
    bind_to: SocketAddr,
) {
    HttpServer::new(move || {
        let api_v01 = api_v01.clone();

        let api_v1_scope = {
            let tx_sender = TxSender::new(
                api_v01.connection_pool.clone(),
                sign_verifier.clone(),
                fee_ticker.clone(),
                &api_v01.config,
            );
            v1::api_scope(tx_sender, &api_v01.config)
        };

        let api_v02_scope = {
            let tx_sender = TxSender::new(
                api_v01.connection_pool.clone(),
                sign_verifier.clone(),
                fee_ticker.clone(),
                &api_v01.config,
            );
            v02::api_scope(tx_sender, &api_v01.config)
        };
        App::new()
            .wrap(Cors::new().send_wildcard().max_age(3600).finish())
            .service(api_v01.into_scope())
            .service(api_v1_scope)
            .service(api_v02_scope.wrap(SayHi))
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
    sign_verifier: mpsc::Sender<VerifySignatureRequest>,
    config: ZkSyncConfig,
) {
    std::thread::Builder::new()
        .name("actix-rest-api".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());

            actix_rt::System::new("api-server").block_on(async move {
                let api_v01 = ApiV01::new(connection_pool, contract_address, config.clone());
                api_v01.spawn_network_status_updater(panic_notify);

                start_server(api_v01, fee_ticker, sign_verifier, listen_addr).await;
            });
        })
        .expect("Api server thread");
}
