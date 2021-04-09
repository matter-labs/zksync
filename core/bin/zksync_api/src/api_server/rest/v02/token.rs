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

// Workspace uses
use zksync_api_types::v02::{
    pagination::{Paginated, PaginationQuery},
    token::ApiToken,
};
use zksync_config::ZkSyncConfig;
use zksync_storage::{ConnectionPool, StorageProcessor};
use zksync_types::{Token, TokenId, TokenLike};

// Local uses
use super::{
    error::{Error, InvalidDataError},
    paginate_trait::Paginate,
    response::ApiResult,
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

    async fn is_token_enabled_for_fees(
        &self,
        storage: &mut StorageProcessor<'_>,
        token_id: TokenId,
    ) -> Result<bool, Error> {
        let result = storage
            .tokens_schema()
            .load_token_ids_that_enabled_for_fees(vec![token_id], &self.min_market_volume)
            .await
            .map_err(Error::storage)?;
        Ok(!result.is_empty())
    }

    async fn token_page(
        &self,
        query: PaginationQuery<TokenId>,
    ) -> Result<Paginated<ApiToken, TokenId>, Error> {
        let mut storage = self.pool.access_storage().await.map_err(Error::storage)?;
        let paginated_tokens: Result<Paginated<Token, TokenId>, Error> =
            storage.paginate(&query).await;
        match paginated_tokens {
            Ok(paginated_tokens) => {
                let mut list = Vec::new();
                let tokens_to_check: Vec<TokenId> =
                    paginated_tokens.list.iter().map(|token| token.id).collect();
                let tokens_enabled_for_fees = storage
                    .tokens_schema()
                    .load_token_ids_that_enabled_for_fees(tokens_to_check, &self.min_market_volume)
                    .await
                    .map_err(Error::storage)?;
                for token in paginated_tokens.list {
                    let enabled_for_fees = tokens_enabled_for_fees.contains(&token.id);
                    list.push(ApiToken::from_token_and_eligibility(
                        token,
                        enabled_for_fees,
                    ));
                }
                Ok(Paginated {
                    list,
                    from: paginated_tokens.from,
                    count: paginated_tokens.count,
                    limit: paginated_tokens.limit,
                    direction: paginated_tokens.direction,
                })
            }
            Err(err) => Err(err),
        }
    }

    async fn token(&self, token_like: TokenLike) -> Result<ApiToken, Error> {
        let mut storage = self.pool.access_storage().await.map_err(Error::storage)?;

        let token = self
            .tokens
            .get_token(&mut storage, token_like)
            .await
            .map_err(Error::storage)?;
        if let Some(token) = token {
            let enabled_for_fees = self
                .is_token_enabled_for_fees(&mut storage, token.id)
                .await?;
            Ok(ApiToken::from_token_and_eligibility(
                token,
                enabled_for_fees,
            ))
        } else {
            Err(Error::from(PriceError::token_not_found(
                "Token not found in storage",
            )))
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
            .map_err(Error::storage)?;

        let price_result = price_receiver.await.map_err(Error::storage)?;
        price_result.map_err(Error::from)
    }
}

// Server implementation

async fn token_pagination(
    data: web::Data<ApiTokenData>,
    web::Query(query): web::Query<PaginationQuery<TokenId>>,
) -> ApiResult<Paginated<ApiToken, TokenId>> {
    data.token_page(query).await.map_err(Error::from).into()
}

async fn token_by_id(
    data: web::Data<ApiTokenData>,
    web::Path(token_like_string): web::Path<String>,
) -> ApiResult<ApiToken> {
    let token_like = TokenLike::parse(&token_like_string);
    let token_like = match token_like {
        TokenLike::Symbol(_) => {
            return Error::from(PriceError::token_not_found(
                "Could not parse token as id or address",
            ))
            .into();
        }
        _ => token_like,
    };

    data.token(token_like).await.into()
}

// TODO: take `currency` as enum.
// Currently actix path extractor doesn't work with enums: https://github.com/actix/actix-web/issues/318
async fn token_price(
    data: web::Data<ApiTokenData>,
    web::Path((token_like_string, currency)): web::Path<(String, String)>,
) -> ApiResult<BigDecimal> {
    let first_token = TokenLike::parse(&token_like_string);
    let first_token = match first_token {
        TokenLike::Symbol(_) => {
            return Error::from(PriceError::token_not_found(
                "Could not parse token as id or address",
            ))
            .into();
        }
        _ => first_token,
    };

    if let Ok(second_token_id) = u16::from_str(&currency) {
        let second_token = TokenLike::from(TokenId(second_token_id));
        let first_usd_price = data.token_price_usd(first_token).await;
        let second_usd_price = data.token_price_usd(second_token).await;
        match (first_usd_price, second_usd_price) {
            (Ok(first_usd_price), Ok(second_usd_price)) => {
                if second_usd_price.is_zero() {
                    Error::from(InvalidDataError::TokenZeroPriceError).into()
                } else {
                    Ok(first_usd_price / second_usd_price).into()
                }
            }
            (Err(err), _) => err.into(),
            (_, Err(err)) => err.into(),
        }
    } else {
        match currency.as_str() {
            "usd" => {
                let usd_price = data.token_price_usd(first_token).await;
                usd_price.into()
            }
            _ => Error::from(InvalidDataError::InvalidCurrency).into(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_server::rest::v02::{
        test_utils::{deserialize_response_result, dummy_fee_ticker, TestServerConfig},
        SharedData,
    };
    use zksync_api_types::v02::{pagination::PaginationDirection, ApiVersion};
    use zksync_types::Address;

    async fn is_token_enabled_for_fees(
        storage: &mut StorageProcessor<'_>,
        token_id: TokenId,
        config: &ZkSyncConfig,
    ) -> anyhow::Result<bool> {
        let market_volume = TokenDBCache::get_token_market_volume(storage, token_id).await?;
        let min_market_volume = Ratio::from(
            BigUint::from_f64(config.ticker.liquidity_volume)
                .expect("TickerConfig::liquidity_volume must be positive"),
        );
        Ok(market_volume
            .map(|volume| volume.market_volume.ge(&min_market_volume))
            .unwrap_or(false))
    }

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn v02_test_token_scope() -> anyhow::Result<()> {
        let cfg = TestServerConfig::default();
        cfg.fill_database().await?;

        let prices = [
            (TokenLike::Id(TokenId(1)), 10_u64.into()),
            (TokenLike::Id(TokenId(15)), 10_500_u64.into()),
            (Address::default().into(), 1_u64.into()),
        ];
        let fee_ticker = dummy_fee_ticker(&prices);

        let shared_data = SharedData {
            net: cfg.config.chain.eth.network,
            api_version: ApiVersion::V02,
        };
        let (client, server) = cfg.start_server(
            move |cfg| {
                api_scope(
                    &cfg.config,
                    cfg.pool.clone(),
                    TokenDBCache::new(),
                    fee_ticker.clone(),
                )
            },
            shared_data,
        );

        let token_like = TokenLike::Id(TokenId(1));
        let response = client.token_by_id_v02(&token_like).await?;
        let api_token: ApiToken = deserialize_response_result(response)?;

        let expected_token = {
            let mut storage = cfg.pool.access_storage().await?;
            storage
                .tokens_schema()
                .get_token(token_like)
                .await?
                .unwrap()
        };
        let expected_enabled_for_fees = {
            let mut storage = cfg.pool.access_storage().await?;
            is_token_enabled_for_fees(&mut storage, TokenId(1), &cfg.config).await?
        };
        let expected_api_token =
            ApiToken::from_token_and_eligibility(expected_token, expected_enabled_for_fees);
        assert_eq!(api_token, expected_api_token);

        let query = PaginationQuery {
            from: TokenId(15),
            limit: 2,
            direction: PaginationDirection::Older,
        };
        let response = client.token_pagination_v02(&query).await?;
        let pagination: Paginated<ApiToken, TokenId> = deserialize_response_result(response)?;

        let expected_pagination = {
            let mut storage = cfg.pool.access_storage().await?;
            let paginated_tokens: Paginated<Token, TokenId> = storage
                .paginate(&query)
                .await
                .map_err(|err| anyhow::anyhow!(err.message))?;
            let mut list = Vec::new();
            for token in paginated_tokens.list {
                let enabled_for_fees =
                    is_token_enabled_for_fees(&mut storage, token.id, &cfg.config).await?;
                list.push(ApiToken::from_token_and_eligibility(
                    token,
                    enabled_for_fees,
                ));
            }
            Paginated {
                list,
                from: paginated_tokens.from,
                count: paginated_tokens.count,
                limit: paginated_tokens.limit,
                direction: paginated_tokens.direction,
            }
        };
        assert_eq!(pagination, expected_pagination);

        let token_like = TokenLike::Id(TokenId(1));
        let response = client.token_price_v02(&token_like, "15").await?;
        let price_in_token: BigDecimal = deserialize_response_result(response)?;
        let expected_price_in_token =
            BigDecimal::from_u32(10).unwrap() / BigDecimal::from_u32(10500).unwrap();
        assert_eq!(price_in_token, expected_price_in_token);

        let response = client.token_price_v02(&token_like, "usd").await?;
        let price_in_usd: BigDecimal = deserialize_response_result(response)?;
        let expected_price_in_usd = BigDecimal::from_u32(10).unwrap();
        assert_eq!(price_in_usd, expected_price_in_usd);

        let response = client.token_price_v02(&token_like, "333").await?;
        assert!(response.error.is_some());

        server.stop().await;
        Ok(())
    }
}
