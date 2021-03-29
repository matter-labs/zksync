//! Tokens part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self},
    Scope,
};
use bigdecimal::{BigDecimal, Zero};
use futures::{
    channel::{mpsc, oneshot},
    prelude::*,
};
use num::{rational::Ratio, BigUint, FromPrimitive};

// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_storage::ConnectionPool;
use zksync_types::{
    pagination::{Paginated, PaginationQuery},
    Token, TokenId, TokenLike,
};

// Local uses
use super::{
    error::{Error, TokenError},
    paginate::Paginate,
    response::ApiResult,
    types::{ApiToken, TokenIdOrUsd, Usd},
};
use crate::{
    fee_ticker::{PriceError, TickerRequest, TokenPriceRequestType},
    utils::token_db_cache::TokenDBCache,
};

/// Shared data between `api/v0.2/tokens` endpoints.
#[derive(Clone)]
struct ApiTokenData {
    min_market_volume: Ratio<BigUint>,
    fee_ticker: mpsc::Sender<TickerRequest>,
    tokens: TokenDBCache,
    pool: ConnectionPool,
}

impl ApiTokenData {
    fn new(
        config: &ZkSyncConfig,
        pool: ConnectionPool,
        tokens: TokenDBCache,
        fee_ticker: mpsc::Sender<TickerRequest>,
    ) -> Self {
        Self {
            min_market_volume: Ratio::from(
                BigUint::from_f64(config.ticker.liquidity_volume)
                    .expect("TickerConfig::liquidity_volume must be positive"),
            ),
            pool,
            tokens,
            fee_ticker,
        }
    }

    async fn token_page(
        &self,
        query: PaginationQuery<TokenId>,
    ) -> Result<Paginated<Token, TokenId>, Error> {
        let mut storage = self.pool.access_storage().await.map_err(Error::internal)?;
        storage.paginate(query).await
    }

    async fn token(&self, token_like: TokenLike) -> Result<ApiToken, Error> {
        let mut storage = self.pool.access_storage().await.map_err(Error::internal)?;

        let token = self
            .tokens
            .get_token(&mut storage, token_like)
            .await
            .map_err(Error::internal)?;
        if let Some(token) = token {
            let market_volume = TokenDBCache::get_token_market_volume(&mut storage, token.id)
                .await
                .map_err(Error::internal)?;
            let mut api_token = ApiToken {
                id: token.id,
                address: token.address,
                symbol: token.symbol,
                decimals: token.decimals,
                enabled_for_fees: false,
            };
            if let Some(market_volume) = market_volume {
                if market_volume.market_volume.ge(&self.min_market_volume) {
                    api_token.enabled_for_fees = true;
                }
            }
            Ok(api_token)
        } else {
            Err(Error::from(PriceError::TokenNotFound(String::from(
                "Token not found",
            ))))
        }
    }

    async fn token_price_usd(&self, token: TokenLike) -> Result<BigDecimal, Error> {
        let (price_sender, price_receiver) = oneshot::channel();
        self.fee_ticker
            .clone()
            .send(TickerRequest::GetTokenPrice {
                token,
                response: price_sender,
                req_type: TokenPriceRequestType::USDForOneToken,
            })
            .await
            .map_err(Error::internal)?;

        let price_result = price_receiver.await.map_err(Error::internal)?;
        price_result.map_err(Error::from)
    }
}

// Server implementation

async fn token_pagination(
    data: web::Data<ApiTokenData>,
    web::Query(query): web::Query<PaginationQuery<TokenId>>,
) -> ApiResult<Paginated<Token, TokenId>> {
    data.token_page(query).await.map_err(Error::from).into()
}

async fn token_by_id(
    data: web::Data<ApiTokenData>,
    web::Path(token_like): web::Path<String>,
) -> ApiResult<ApiToken> {
    let token_result = TokenLike::parse_without_symbol(&token_like);
    let token_like;
    if let Err(err) = token_result {
        return Error::from(PriceError::TokenNotFound(err.to_string())).into();
    } else {
        token_like = token_result.unwrap();
    }

    data.token(token_like).await.into()
}

async fn token_price(
    data: web::Data<ApiTokenData>,
    web::Path((token_like, currency)): web::Path<(String, TokenIdOrUsd)>,
) -> ApiResult<BigDecimal> {
    let token_result = TokenLike::parse_without_symbol(&token_like);
    let first_token;
    if let Err(_) = token_result {
        return Error::from(TokenError::TokenNotFound).into();
    } else {
        first_token = token_result.unwrap();
    }

    match currency {
        TokenIdOrUsd::Id(second_token_id) => {
            let second_token = TokenLike::from(second_token_id);
            let first_usd_price = data.token_price_usd(first_token).await;
            let second_usd_price = data.token_price_usd(second_token).await;
            match (first_usd_price, second_usd_price) {
                (Ok(first_usd_price), Ok(second_usd_price)) => {
                    if second_usd_price.is_zero() {
                        Error::from(TokenError::ZeroPrice).into()
                    } else {
                        Ok(first_usd_price / second_usd_price).into()
                    }
                }
                (Err(err), _) => err.into(),
                (_, Err(err)) => err.into(),
            }
        }
        TokenIdOrUsd::Usd(Usd::Usd) => {
            let usd_price = data.token_price_usd(first_token).await;
            usd_price.into()
        }
    }
}

pub fn api_scope(
    config: &ZkSyncConfig,
    pool: ConnectionPool,
    tokens_db: TokenDBCache,
    fee_ticker: mpsc::Sender<TickerRequest>,
) -> Scope {
    let data = ApiTokenData::new(config, pool, tokens_db, fee_ticker);

    web::scope("token")
        .data(data)
        .route("", web::get().to(token_pagination))
        .route("{token_id}", web::get().to(token_by_id))
        .route("{token_id}/price_in/{currency}", web::get().to(token_price))
}
