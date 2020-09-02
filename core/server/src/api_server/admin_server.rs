// Built-in deps
use models::node::Address;
use models::node::TokenId;
use std::net::SocketAddr;
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
use jsonwebtoken::errors::Error as JwtError;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

// Local uses
use models::config_options::ThreadPanicNotify;
use models::node::tokens;

#[derive(Debug, Serialize, Deserialize)]
struct PayloadAuthToken {
    sub: String, // Subject (whom auth token refers to)
    exp: usize,  // Expiration time (as UTC timestamp)
}

/// Validate JsonWebToken
pub fn validate_auth_token(token: &str, secret: &str) -> Result<(), JwtError> {
    let token = decode::<PayloadAuthToken>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    );

    match token {
        Ok(_data) => Ok(()),
        Err(err) => Err(err),
    }
}

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

fn add_token(
    data: web::Data<AppState>,
    r: web::Json<tokens::AddTokenRequest>,
) -> actix_web::Result<HttpResponse> {
    let storage = data.access_storage()?;

    // if id is None then set it to next available ID from server.
    let id = match r.id {
        Some(id) => id,
        None => storage.tokens_schema().get_count().map_err(|e| {
            vlog::warn!(
                "failed get number of token from database in progress request: {}",
                e
            );
            actix_web::error::ErrorInternalServerError("storage layer error")
        })? as u16,
    };

    let token = tokens::Token {
        id,
        address: r.address,
        symbol: r.symbol.clone(),
        decimals: r.decimals,
    };

    storage
        .tokens_schema()
        .store_token(token.clone())
        .map_err(|e| {
            vlog::warn!("failed add token to database in progress request: {}", e);
            actix_web::error::ErrorInternalServerError("storage layer error")
        })?;

    Ok(HttpResponse::Ok().json(token))
}

fn validator(
    req: ServiceRequest,
    credentials: BearerAuth,
    secret_auth: &str,
) -> Result<ServiceRequest, Error> {
    let config = req
        .app_data::<Config>()
        .map(|data| data.get_ref().clone())
        .unwrap_or_else(Default::default);

    validate_auth_token(credentials.token(), secret_auth)
        .map(|_| req)
        .map_err(|_| AuthenticationError::from(config).into())
}

pub fn start_admin_server(
    bind_to: SocketAddr,
    secret_auth: String,
    connection_pool: storage::ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
) {
    let secret_auth: &'static _ = Box::leak(Box::new(secret_auth));

    thread::Builder::new()
        .name("admin_server".to_string())
        .spawn(move || {
            HttpServer::new(move || {
                let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());

                let app_state = AppState {
                    connection_pool: connection_pool.clone(),
                };

                let auth = HttpAuthentication::bearer(move |req, credentials| {
                    validator(req, credentials, &secret_auth)
                });

                App::new()
                    .wrap(auth)
                    .register_data(web::Data::new(app_state))
                    .route("/tokens", web::post().to(add_token))
            })
            .workers(1)
            .bind(&bind_to)
            .expect("failed to bind")
            .run()
            .expect("failed to run endpoint server");
        })
        .expect("failed to start endpoint server");
}
