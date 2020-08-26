use crate::api_server::auth;

// Built-in deps
use std::thread;

// External uses
use actix_web::dev::ServiceRequest;
use actix_web::{web, App, Error, HttpResponse, HttpServer};
use actix_web_httpauth::extractors::{
    bearer::{BearerAuth, Config},
    AuthenticationError,
};
use actix_web_httpauth::middleware::HttpAuthentication;
use futures::channel::mpsc;

// Local uses
use models::config_options::{ConfigurationOptions, ThreadPanicNotify};

#[derive(Debug)]
struct AppState {
    connection_pool: storage::ConnectionPool,
}

impl AppState {
    fn access_storage(&self) -> actix_web::Result<storage::StorageProcessor> {
        self.connection_pool.access_storage_fragile().map_err(|e| {
            vlog::warn!("Failed to access storage: {}", e);
            actix_web::error::ErrorInternalServerError(e)
        })
    }
}

fn get_tokens(data: web::Data<AppState>) -> actix_web::Result<HttpResponse> {
    let storage = data.access_storage()?;
    let res = storage.tokens_schema().load_tokens().map_err(|e| {
        vlog::warn!("could not get token: {}", e);
        actix_web::error::ErrorInternalServerError("storage layer error")
    })?;

    let res = res.values().cloned().collect::<Vec<_>>();
    Ok(HttpResponse::Ok().json(res))
}

fn add_token(
    data: web::Data<AppState>,
    r: web::Json<models::node::tokens::Token>,
) -> actix_web::Result<()> {
    let storage = data.access_storage()?;

    storage.tokens_schema().store_token(r.0).map_err(|e| {
        vlog::warn!("failed add token to database in progress request: {}", e);
        actix_web::error::ErrorInternalServerError("storage layer error")
    })
}

fn get_number_of_token(data: web::Data<AppState>) -> actix_web::Result<HttpResponse> {
    let storage = data.access_storage()?;

    let res = storage.tokens_schema().get_count().map_err(|e| {
        vlog::warn!(
            "failed get number of token from database in progress request: {}",
            e
        );
        actix_web::error::ErrorInternalServerError("storage layer error")
    })?;

    Ok(HttpResponse::Ok().json(res))
}

fn validator(req: ServiceRequest, credentials: BearerAuth) -> Result<ServiceRequest, Error> {
    let config = req
        .app_data::<Config>()
        .map(|data| data.get_ref().clone())
        .unwrap_or_else(Default::default);

    match auth::validate_token(credentials.token()) {
        Ok(res) => {
            if res {
                Ok(req)
            } else {
                Err(AuthenticationError::from(config).into())
            }
        }
        Err(_) => Err(AuthenticationError::from(config).into()),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn start_endpoint_server(
    config_options: &ConfigurationOptions,
    connection_pool: storage::ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
) {
    let bind_to = config_options.endpoint_http_server_address;

    thread::Builder::new()
        .name("endpoint_server".to_string())
        .spawn(move || {
            HttpServer::new(move || {
                let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());

                let app_state = AppState {
                    connection_pool: connection_pool.clone(),
                };

                let auth = HttpAuthentication::bearer(validator);

                App::new()
                    .wrap(auth)
                    .register_data(web::Data::new(app_state))
                    .route("/count", web::get().to(get_number_of_token))
                    .route("/tokens", web::post().to(add_token))
                    .route("/tokens", web::get().to(get_tokens))
            })
            .bind(&bind_to)
            .expect("failed to bind")
            .run()
            .expect("failed to run endpoint server");
        })
        .expect("failed to start endpoint server");
}
