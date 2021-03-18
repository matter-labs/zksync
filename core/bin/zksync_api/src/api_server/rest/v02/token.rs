//! Tokens part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self},
    Scope,
};
use futures::channel::mpsc;
use num::{rational::Ratio, BigUint, FromPrimitive};
use serde::{Deserialize, Serialize};

use zksync_config::ZkSyncConfig;
// Workspace uses
use zksync_storage::{ConnectionPool, QueryResult};
use zksync_types::{Address, TokenId, TokenLike};

use crate::{fee_ticker::TickerRequest, utils::token_db_cache::TokenDBCache};

// Local uses
use super::{error::InternalError, response::ApiResult};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
struct ApiToken {
    pub id: TokenId,
    pub address: Address,
    pub symbol: String,
    pub decimals: u8,
    pub enabled_for_fees: bool,
}

/// Shared data between `api/v02/tokens` endpoints.
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
}

// Server implementation

async fn token_by_id(
    data: web::Data<ApiTokenData>,
    web::Path(token_like): web::Path<String>,
) -> ApiResult<Option<ApiToken>, InternalError> {
    let token_like = TokenLike::parse(&token_like);

    data.token(token_like)
        .await
        .map_err(InternalError::new)
        .into()
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
}
