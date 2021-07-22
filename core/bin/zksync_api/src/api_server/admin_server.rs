// Built-in deps
use std::net::SocketAddr;
use std::thread;

// External uses
use actix_web::dev::ServiceRequest;
use actix_web::{web, App, HttpResponse, HttpServer};
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
use zksync_storage::ConnectionPool;
use zksync_types::{tokens, Address, TokenId};
use zksync_utils::panic_notify::ThreadPanicNotify;

#[derive(Debug, Serialize, Deserialize)]
struct PayloadAuthToken {
    /// Subject (whom auth token refers to).
    sub: String,
    /// Expiration time (as UTC timestamp).
    exp: usize,
}

#[derive(Debug, Clone)]
struct AppState {
    secret_auth: String,
    connection_pool: ConnectionPool,
}

impl AppState {
    async fn access_storage(&self) -> actix_web::Result<zksync_storage::StorageProcessor<'_>> {
        self.connection_pool.access_storage().await.map_err(|e| {
            vlog::warn!("Failed to access storage: {}", e);
            actix_web::error::ErrorInternalServerError(e)
        })
    }
}

/// Token that contains information to add to the server
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
struct AddTokenRequest {
    /// id is used for tx signature and serialization
    /// is optional because when adding the server will assign the next available ID
    pub id: Option<TokenId>,
    /// Contract address of ERC20 token or Address::zero() for "ETH"
    pub address: Address,
    /// Token symbol (e.g. "ETH" or "USDC")
    pub symbol: String,
    /// Token precision (e.g. 18 for "ETH" so "1.0" ETH = 10e18 as U256 number)
    pub decimals: u8,
}

struct AuthTokenValidator<'a> {
    decoding_key: DecodingKey<'a>,
}

impl<'a> AuthTokenValidator<'a> {
    fn new(secret: &'a str) -> Self {
        Self {
            decoding_key: DecodingKey::from_secret(secret.as_ref()),
        }
    }

    /// Validate JsonWebToken
    fn validate_auth_token(&self, token: &str) -> Result<(), JwtError> {
        decode::<PayloadAuthToken>(token, &self.decoding_key, &Validation::default())?;

        Ok(())
    }

    async fn validator(
        &self,
        req: ServiceRequest,
        credentials: BearerAuth,
    ) -> actix_web::Result<ServiceRequest> {
        let config = req.app_data::<Config>().cloned().unwrap_or_default();

        self.validate_auth_token(credentials.token())
            .map_err(|_| AuthenticationError::from(config))?;

        Ok(req)
    }
}

async fn add_token(
    data: web::Data<AppState>,
    token_request: web::Json<AddTokenRequest>,
) -> actix_web::Result<HttpResponse> {
    let mut storage = data.access_storage().await?;

    // if id is None then set it to next available ID from server.
    let id = match token_request.id {
        Some(id) => id,
        None => {
            let last_token_id = storage.tokens_schema().get_count().await.map_err(|e| {
                vlog::warn!(
                    "failed get number of token from database in progress request: {}",
                    e
                );
                actix_web::error::ErrorInternalServerError("storage layer error")
            })?;
            let next_available_id = last_token_id + 1;

            TokenId(next_available_id)
        }
    };

    let token = tokens::Token {
        id,
        address: token_request.address,
        symbol: token_request.symbol.clone(),
        decimals: token_request.decimals,
        is_nft: false,
    };

    storage
        .tokens_schema()
        .store_or_update_token(token.clone())
        .await
        .map_err(|e| {
            vlog::warn!("failed add token to database in progress request: {}", e);
            actix_web::error::ErrorInternalServerError("storage layer error")
        })?;

    Ok(HttpResponse::Ok().json(token))
}

async fn run_server(app_state: AppState, bind_to: SocketAddr) {
    HttpServer::new(move || {
        let auth = HttpAuthentication::bearer(move |req, credentials| async {
            let secret_auth = req
                .app_data::<web::Data<AppState>>()
                .expect("failed get AppState upon receipt of the authentication token")
                .secret_auth
                .clone();
            AuthTokenValidator::new(&secret_auth)
                .validator(req, credentials)
                .await
        });

        App::new()
            .wrap(auth)
            .wrap(vlog::actix_middleware())
            .app_data(web::Data::new(app_state.clone()))
            .route("/tokens", web::post().to(add_token))
    })
    .workers(1)
    .bind(&bind_to)
    .expect("failed to bind")
    .run()
    .await
    .expect("failed to run endpoint server");
}

pub fn start_admin_server(
    bind_to: SocketAddr,
    secret_auth: String,
    connection_pool: zksync_storage::ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
) {
    thread::Builder::new()
        .name("admin_server".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());
            actix_rt::System::new("api-server").block_on(async move {
                let app_state = AppState {
                    secret_auth,
                    connection_pool,
                };

                run_server(app_state, bind_to).await;
            });
        })
        .expect("failed to start endpoint server");
}
