//! Tokens part of API implementation.

// Built-in uses
use std::str::FromStr;

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
use serde::{Deserialize, Serialize};

use zksync_config::ZkSyncConfig;
// Workspace uses
use zksync_storage::{ConnectionPool, QueryResult};
use zksync_types::{Address, TokenId, TokenLike};

// Local uses
use super::{error::InternalError, response::ApiResult};
use crate::{
    fee_ticker::{PriceError, TickerRequest, TokenPriceRequestType},
    utils::token_db_cache::TokenDBCache,
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
struct ApiToken {
    pub id: TokenId,
    pub address: Address,
    pub symbol: String,
    pub decimals: u8,
    pub enabled_for_fees: bool,
}

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

    async fn token(&self, token_like: TokenLike) -> QueryResult<Option<ApiToken>> {
        let mut storage = self.pool.access_storage().await?;

        let token = self.tokens.get_token(&mut storage, token_like).await?;
        if let Some(token) = token {
            let market_volume =
                TokenDBCache::get_token_market_volume(&mut storage, token.id).await?;
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
            Ok(Some(api_token))
        } else {
            Ok(None)
        }
    }

    async fn token_price_usd(&self, token: TokenLike) -> QueryResult<Option<BigDecimal>> {
        let (price_sender, price_receiver) = oneshot::channel();
        self.fee_ticker
            .clone()
            .send(TickerRequest::GetTokenPrice {
                token,
                response: price_sender,
                req_type: TokenPriceRequestType::USDForOneToken,
            })
            .await?;

        match price_receiver.await? {
            Ok(price) => Ok(Some(price)),
            Err(PriceError::TokenNotFound(_)) => Ok(None),
            Err(PriceError::DBError(err)) => Err(anyhow::format_err!(err)),
            Err(PriceError::ApiError(err)) => Err(anyhow::format_err!(err)),
        }
    }
}

// Server implementation

async fn token_by_id(
    data: web::Data<ApiTokenData>,
    web::Path(token_like): web::Path<String>,
) -> ApiResult<Option<ApiToken>, InternalError> {
    let token_result = TokenLike::parse_without_symbol(&token_like);
    let token_like;
    if let Err(err) = token_result {
        return InternalError::new(err).into();
    } else {
        token_like = token_result.unwrap();
    }

    data.token(token_like)
        .await
        .map_err(InternalError::new)
        .into()
}

async fn token_price(
    data: web::Data<ApiTokenData>,
    web::Path((token_like, currency)): web::Path<(String, String)>,
) -> ApiResult<Option<BigDecimal>, InternalError> {
    let token_result = TokenLike::parse_without_symbol(&token_like);
    let first_token;
    if let Err(err) = token_result {
        return InternalError::new(err).into();
    } else {
        first_token = token_result.unwrap();
    }

    if let Ok(second_token_id) = u16::from_str(&currency) {
        let second_token = TokenLike::from(TokenId(second_token_id));
        let first_usd_price = data.token_price_usd(first_token).await;
        let second_usd_price = data.token_price_usd(second_token).await;
        if first_usd_price.is_ok() && second_usd_price.is_ok() {
            match (first_usd_price.unwrap(), second_usd_price.unwrap()) {
                (None, _) => Ok(None).into(),
                (_, None) => Ok(None).into(),
                (Some(first_usd_price), Some(second_usd_price)) => {
                    if second_usd_price.is_zero() {
                        InternalError::new("Price of token in which the price is indicated is zero")
                            .into()
                    } else {
                        Ok(Some(first_usd_price / second_usd_price)).into()
                    }
                }
            }
        } else {
            InternalError::new("Error getting token usd price").into()
        }
    } else {
        let usd_price = match currency.as_str() {
            "usd" => data.token_price_usd(first_token).await,
            _ => Err(anyhow::anyhow!("There are only {token_id} and usd options")),
        };
        usd_price.map_err(InternalError::new).into()
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
        .route("{token_id}", web::get().to(token_by_id))
        .route("{token_id}/price_in/{currency}", web::get().to(token_price))
}
