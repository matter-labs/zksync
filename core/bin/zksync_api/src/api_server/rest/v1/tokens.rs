//! Tokens part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};
use bigdecimal::BigDecimal;
use futures::{
    channel::{mpsc, oneshot},
    prelude::*,
};

// Workspace uses
use zksync_api_client::rest::v1::{TokenPriceKind, TokenPriceQuery};
use zksync_storage::{ConnectionPool, QueryResult};
use zksync_types::{Token, TokenLike};

use crate::{
    fee_ticker::{PriceError, TickerRequest, TokenPriceRequestType},
    utils::token_db_cache::TokenDBCache,
};

// Local uses
use super::{ApiError, JsonResult};

/// Shared data between `api/v1/tokens` endpoints.
#[derive(Clone)]
struct ApiTokensData {
    fee_ticker: mpsc::Sender<TickerRequest>,
    tokens: TokenDBCache,
    pool: ConnectionPool,
}

impl ApiTokensData {
    fn new(
        pool: ConnectionPool,
        tokens: TokenDBCache,
        fee_ticker: mpsc::Sender<TickerRequest>,
    ) -> Self {
        Self {
            fee_ticker,
            tokens,
            pool,
        }
    }

    async fn tokens(&self) -> QueryResult<Vec<Token>> {
        let mut storage = self.pool.access_storage().await?;

        let tokens = storage.tokens_schema().load_tokens().await?;

        // Provide tokens in a predictable order.
        let mut tokens: Vec<_> = tokens.into_iter().map(|(_k, v)| v).collect();
        tokens.sort_unstable_by_key(|token| token.id);

        Ok(tokens)
    }

    async fn token(&self, token_like: TokenLike) -> QueryResult<Option<Token>> {
        let mut storage = self.pool.access_storage().await?;

        self.tokens.get_token(&mut storage, token_like).await
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

async fn tokens(data: web::Data<ApiTokensData>) -> JsonResult<Vec<Token>> {
    let tokens = data.tokens().await.map_err(ApiError::internal)?;

    Ok(Json(tokens))
}

async fn token_by_id(
    data: web::Data<ApiTokensData>,
    web::Path(token_like): web::Path<String>,
) -> JsonResult<Option<Token>> {
    let token_like = TokenLike::parse(&token_like);

    let token = data.token(token_like).await.map_err(ApiError::internal)?;
    Ok(Json(token))
}

async fn token_price(
    data: web::Data<ApiTokensData>,
    web::Path(token_like): web::Path<String>,
    web::Query(token_query): web::Query<TokenPriceQuery>,
) -> JsonResult<Option<BigDecimal>> {
    let token_like = TokenLike::parse(&token_like);

    let price = match token_query.kind {
        TokenPriceKind::Currency => data
            .token_price_usd(token_like)
            .await
            .map_err(ApiError::internal)?,

        TokenPriceKind::Token => {
            return Err(ApiError::not_implemented(
                "price in tokens not yet implemented",
            ))
        }
    };

    Ok(Json(price))
}

pub fn api_scope(
    pool: ConnectionPool,
    tokens_db: TokenDBCache,
    fee_ticker: mpsc::Sender<TickerRequest>,
) -> Scope {
    let data = ApiTokensData::new(pool, tokens_db, fee_ticker);

    web::scope("tokens")
        .data(data)
        .route("", web::get().to(tokens))
        .route("{id}", web::get().to(token_by_id))
        .route("{id}/price", web::get().to(token_price))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use zksync_types::{Address, TokenId};

    use super::{super::test_utils::TestServerConfig, *};

    use zksync_api_client::rest::v1::ClientError;

    fn dummy_fee_ticker(prices: &[(TokenLike, BigDecimal)]) -> mpsc::Sender<TickerRequest> {
        let (sender, mut receiver) = mpsc::channel(10);

        let prices: HashMap<_, _> = prices.iter().cloned().collect();
        actix_rt::spawn(async move {
            while let Some(item) = receiver.next().await {
                match item {
                    TickerRequest::GetTokenPrice {
                        token,
                        response,
                        req_type,
                    } => {
                        assert_eq!(
                            req_type,
                            TokenPriceRequestType::USDForOneToken,
                            "Unsupported price request type"
                        );

                        let msg = if let Some(price) = prices.get(&token) {
                            Ok(price.clone())
                        } else {
                            Err(PriceError::token_not_found(format!(
                                "Token not found: {:?}",
                                token
                            )))
                        };

                        response.send(msg).expect("Unable to send response");
                    }
                    _ => unreachable!("Unsupported request"),
                }
            }
        });

        sender
    }

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn test_tokens_scope() -> anyhow::Result<()> {
        let cfg = TestServerConfig::default();
        cfg.fill_database().await?;

        let prices = [
            (TokenLike::Id(TokenId(1)), 10_u64.into()),
            (TokenLike::Id(TokenId(15)), 10_500_u64.into()),
            ("ETH".into(), 0_u64.into()),
            (Address::default().into(), 1_u64.into()),
        ];
        let fee_ticker = dummy_fee_ticker(&prices);

        let (client, server) = cfg.start_server(move |cfg| {
            api_scope(cfg.pool.clone(), TokenDBCache::new(), fee_ticker.clone())
        });

        // Fee requests
        for (token, expected_price) in &prices {
            let actual_price = client.token_price(token, TokenPriceKind::Currency).await?;

            assert_eq!(
                actual_price.as_ref(),
                Some(expected_price),
                "Price does not match"
            );
        }
        assert_eq!(
            client
                .token_price(&TokenLike::Id(TokenId(2)), TokenPriceKind::Currency)
                .await?,
            None
        );
        let error = client
            .token_price(&TokenLike::Id(TokenId(2)), TokenPriceKind::Token)
            .await
            .unwrap_err();
        assert!(
            matches!(error, ClientError::BadRequest { .. }),
            "Incorrect error type: got {:?} instead of BadRequest",
            error
        );
        // Tokens requests
        let expected_tokens = {
            let mut storage = cfg.pool.access_storage().await?;

            let mut tokens: Vec<_> = storage
                .tokens_schema()
                .load_tokens()
                .await?
                .values()
                .cloned()
                .collect();
            tokens.sort_unstable_by(|lhs, rhs| lhs.id.cmp(&rhs.id));
            tokens
        };

        assert_eq!(client.tokens().await?, expected_tokens);

        let expected_token = &expected_tokens[0];
        assert_eq!(
            &client
                .token_by_id(&TokenLike::Id(TokenId(0)))
                .await?
                .unwrap(),
            expected_token
        );
        assert_eq!(
            &client
                .token_by_id(&TokenLike::parse(
                    "0x0000000000000000000000000000000000000000"
                ))
                .await?
                .unwrap(),
            expected_token
        );
        assert_eq!(
            &client
                .token_by_id(&TokenLike::parse(
                    "0000000000000000000000000000000000000000"
                ))
                .await?
                .unwrap(),
            expected_token
        );
        assert_eq!(
            &client.token_by_id(&TokenLike::parse("ETH")).await?.unwrap(),
            expected_token
        );
        assert_eq!(client.token_by_id(&TokenLike::parse("XM")).await?, None);

        server.stop().await;
        Ok(())
    }
}
