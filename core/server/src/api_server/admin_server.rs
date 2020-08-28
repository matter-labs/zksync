// Built-in deps
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
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

// Local uses
use models::config_options::ThreadPanicNotify;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
}

/// Encode JsonWebToken with shared secret - secret,
/// sub - message and exp - time until token will be valid
pub fn encode_token(secret: &str, sub: &str, exp: usize) -> Result<String, JwtError> {
    let claim = Claims {
        sub: sub.to_string(),
        exp,
    };
    encode(
        &Header::default(),
        &claim,
        &EncodingKey::from_secret(secret.as_ref()),
    )
}

/// Validate JsonWebToken
pub fn validate_token(token: &str, secret: &str) -> Result<bool, JwtError> {
    let token = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    );

    match token {
        Ok(_data) => Ok(true),
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

fn validator(
    req: ServiceRequest,
    credentials: BearerAuth,
    secret_auth: &str,
) -> Result<ServiceRequest, Error> {
    let config = req
        .app_data::<Config>()
        .map(|data| data.get_ref().clone())
        .unwrap_or_else(Default::default);

    match validate_token(credentials.token(), secret_auth) {
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
                    validator(req, credentials, secret_auth)
                });

                App::new()
                    .wrap(auth)
                    .register_data(web::Data::new(app_state))
                    .route("/count", web::get().to(get_number_of_token))
                    .route("/tokens", web::post().to(add_token))
            })
            .bind(&bind_to)
            .expect("failed to bind")
            .run()
            .expect("failed to run endpoint server");
        })
        .expect("failed to start endpoint server");
}
