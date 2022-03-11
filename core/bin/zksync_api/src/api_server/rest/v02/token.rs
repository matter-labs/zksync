//! Tokens part of API implementation.

// Built-in uses
use std::str::FromStr;
use std::time::Instant;

// External uses
use actix_web::{
    web::{self},
    Scope,
};
use bigdecimal::{BigDecimal, Zero};
use num::{rational::Ratio, BigUint, FromPrimitive};

// Workspace uses
use zksync_api_types::v02::{
    pagination::{parse_query, ApiEither, Paginated, PaginationQuery},
    token::{ApiNFT, ApiToken, TokenPrice},
};
use zksync_config::ZkSyncConfig;
use zksync_crypto::params::MIN_NFT_TOKEN_ID;
use zksync_storage::{ConnectionPool, StorageProcessor};
use zksync_types::{tx::TxHash, AccountId, Token, TokenId, TokenLike};

// Local uses
use super::{
    error::{Error, InvalidDataError},
    paginate_trait::Paginate,
    response::ApiResult,
};
use crate::{
    api_try,
    fee_ticker::{FeeTicker, PriceError, TokenPriceRequestType},
    utils::token_db_cache::TokenDBCache,
};

/// Shared data between `api/v0.2/tokens` endpoints.
#[derive(Clone)]
struct ApiTokenData {
    min_market_volume: Ratio<BigUint>,
    fee_ticker: FeeTicker,
    tokens: TokenDBCache,
    pool: ConnectionPool,
}

impl ApiTokenData {
    fn new(
        config: &ZkSyncConfig,
        pool: ConnectionPool,
        tokens: TokenDBCache,
        fee_ticker: FeeTicker,
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
}

impl ApiTokenData {
    async fn is_token_enabled_for_fees(
        &self,
        storage: &mut StorageProcessor<'_>,
        token_id: TokenId,
    ) -> Result<bool, Error> {
        let result = storage
            .tokens_schema()
            .filter_tokens_by_market_volume(vec![token_id], &self.min_market_volume)
            .await
            .map_err(Error::storage)?;
        Ok(!result.is_empty())
    }

    async fn token_page(
        &self,
        query: PaginationQuery<ApiEither<TokenId>>,
    ) -> Result<Paginated<ApiToken, TokenId>, Error> {
        let mut storage = self.pool.access_storage().await.map_err(Error::storage)?;
        let paginated_tokens: Result<Paginated<Token, TokenId>, Error> =
            storage.paginate_checked(&query).await;
        match paginated_tokens {
            Ok(paginated_tokens) => {
                let tokens_to_check: Vec<TokenId> =
                    paginated_tokens.list.iter().map(|token| token.id).collect();
                let tokens_enabled_for_fees = storage
                    .tokens_schema()
                    .filter_tokens_by_market_volume(tokens_to_check, &self.min_market_volume)
                    .await
                    .map_err(Error::storage)?;
                let list = paginated_tokens
                    .list
                    .into_iter()
                    .map(|token| {
                        let eligibility = tokens_enabled_for_fees.contains(&token.id);
                        ApiToken::from_token_and_eligibility(token, eligibility)
                    })
                    .collect();
                Ok(Paginated::new(
                    list,
                    paginated_tokens.pagination.from,
                    paginated_tokens.pagination.limit,
                    paginated_tokens.pagination.direction,
                    paginated_tokens.pagination.count,
                ))
            }
            Err(err) => Err(err),
        }
    }

    async fn token(&self, token_like: TokenLike) -> Result<Token, Error> {
        // Try to find the token in the cache first.
        if let Some(token) = self
            .tokens
            .try_get_token_from_cache(token_like.clone())
            .await
        {
            return Ok(token);
        }

        // Establish db connection and repeat the query, so the token is loaded
        // from the db.
        let mut storage = self.pool.access_storage().await.map_err(Error::storage)?;

        let token = self
            .tokens
            .get_token(&mut storage, token_like)
            .await
            .map_err(Error::storage)?;
        if let Some(token) = token {
            Ok(token)
        } else {
            Err(Error::from(PriceError::token_not_found(
                "Token not found in storage",
            )))
        }
    }

    async fn api_token(&self, token_like: TokenLike) -> Result<ApiToken, Error> {
        let token = self.token(token_like).await?;
        let mut storage = self.pool.access_storage().await.map_err(Error::storage)?;
        let enabled_for_fees = self
            .is_token_enabled_for_fees(&mut storage, token.id)
            .await?;
        Ok(ApiToken::from_token_and_eligibility(
            token,
            enabled_for_fees,
        ))
    }

    async fn token_price_usd(&self, token: TokenLike) -> Result<BigDecimal, Error> {
        self.fee_ticker
            .get_token_price(token, TokenPriceRequestType::USDForOneToken)
            .await
            .map_err(Error::storage)
    }
    // TODO: take `currency` as enum. (ZKS-628)
    async fn token_price_in(
        &self,
        first_token: TokenLike,
        currency: &str,
    ) -> Result<BigDecimal, Error> {
        if let Ok(second_token_id) = u32::from_str(currency) {
            let second_token = TokenLike::from(TokenId(second_token_id));
            let first_usd_price = self.token_price_usd(first_token).await;
            let second_usd_price = self.token_price_usd(second_token).await;
            match (first_usd_price, second_usd_price) {
                (Ok(first_usd_price), Ok(second_usd_price)) => {
                    if second_usd_price.is_zero() {
                        Err(Error::from(InvalidDataError::TokenZeroPriceError))
                    } else {
                        Ok(first_usd_price / second_usd_price)
                    }
                }
                (Err(err), _) => Err(err),
                (_, Err(err)) => Err(err),
            }
        } else {
            match currency {
                "usd" => self.token_price_usd(first_token).await,
                _ => Err(Error::from(InvalidDataError::InvalidCurrency)),
            }
        }
    }
}

// Server implementation

async fn token_pagination(
    data: web::Data<ApiTokenData>,
    web::Query(query): web::Query<PaginationQuery<String>>,
) -> ApiResult<Paginated<ApiToken, TokenId>> {
    let start = Instant::now();
    let query = api_try!(parse_query(query).map_err(Error::from));
    let res = data.token_page(query).await.into();
    metrics::histogram!("api", start.elapsed(), "type" => "v02", "endpoint_name" => "token_pagination");
    res
}

async fn token_info(
    data: web::Data<ApiTokenData>,
    token_like_string: web::Path<String>,
) -> ApiResult<ApiToken> {
    let start = Instant::now();
    let token_like = TokenLike::parse(&token_like_string);
    let res = data.api_token(token_like).await.into();
    metrics::histogram!("api", start.elapsed(), "type" => "v02", "endpoint_name" => "token_info");
    res
}

// TODO: take `currency` as enum.
// Currently actix path extractor doesn't work with enums: https://github.com/actix/actix-web/issues/318 (ZKS-628)
async fn token_price(
    data: web::Data<ApiTokenData>,
    path: web::Path<(String, String)>,
) -> ApiResult<TokenPrice> {
    let start = Instant::now();
    let (token_like_string, currency) = path.into_inner();
    let first_token = TokenLike::parse(&token_like_string);

    let price = api_try!(data.token_price_in(first_token.clone(), &currency).await);
    let token = api_try!(data.token(first_token).await);

    metrics::histogram!("api", start.elapsed(), "type" => "v02", "endpoint_name" => "get_token_price");
    ApiResult::Ok(TokenPrice {
        token_id: token.id,
        token_symbol: token.symbol,
        price_in: currency.to_string(),
        decimals: token.decimals,
        price,
    })
}

async fn get_nft(
    data: web::Data<ApiTokenData>,
    id: web::Path<TokenId>,
) -> ApiResult<Option<ApiNFT>> {
    let start = Instant::now();
    if id.0 < MIN_NFT_TOKEN_ID {
        return Error::from(InvalidDataError::InvalidNFTTokenId).into();
    }
    let mut storage = api_try!(data.pool.access_storage().await.map_err(Error::storage));
    let nft = api_try!(storage
        .tokens_schema()
        .get_nft_with_factories(*id)
        .await
        .map_err(Error::storage));
    metrics::histogram!("api", start.elapsed(), "type" => "v02", "endpoint_name" => "get_nft");
    ApiResult::Ok(nft)
}

async fn get_nft_owner(
    data: web::Data<ApiTokenData>,
    id: web::Path<TokenId>,
) -> ApiResult<Option<AccountId>> {
    let start = Instant::now();
    if id.0 < MIN_NFT_TOKEN_ID {
        return Error::from(InvalidDataError::InvalidNFTTokenId).into();
    }
    let mut storage = api_try!(data.pool.access_storage().await.map_err(Error::storage));
    let owner_id = api_try!(storage
        .chain()
        .account_schema()
        .get_nft_owner(*id)
        .await
        .map_err(Error::storage));
    metrics::histogram!("api", start.elapsed(), "type" => "v02", "endpoint_name" => "get_nft_owner");
    ApiResult::Ok(owner_id)
}

async fn get_nft_id_by_tx_hash(
    data: web::Data<ApiTokenData>,
    tx_hash: web::Path<TxHash>,
) -> ApiResult<Option<TokenId>> {
    let start = Instant::now();
    let mut storage = api_try!(data.pool.access_storage().await.map_err(Error::storage));
    let nft_id = api_try!(storage
        .chain()
        .state_schema()
        .get_nft_id_by_tx_hash(*tx_hash)
        .await
        .map_err(Error::storage));
    metrics::histogram!("api", start.elapsed(), "type" => "v02", "endpoint_name" => "get_nft_id_by_tx_hash");
    ApiResult::Ok(nft_id)
}

pub fn api_scope(
    config: &ZkSyncConfig,
    pool: ConnectionPool,
    tokens_db: TokenDBCache,
    fee_ticker: FeeTicker,
) -> Scope {
    let data = ApiTokenData::new(config, pool, tokens_db, fee_ticker);

    web::scope("tokens")
        .app_data(web::Data::new(data))
        .route("", web::get().to(token_pagination))
        .route("{token_like}", web::get().to(token_info))
        .route(
            "{token_like}/priceIn/{currency}",
            web::get().to(token_price),
        )
        .route("nft/{id}", web::get().to(get_nft))
        .route("nft/{id}/owner", web::get().to(get_nft_owner))
        .route(
            "nft_id_by_tx_hash/{tx_hash}",
            web::get().to(get_nft_id_by_tx_hash),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_server::rest::v02::{
        test_utils::{deserialize_response_result, dummy_fee_ticker, TestServerConfig},
        SharedData,
    };
    use zksync_api_types::v02::{pagination::PaginationDirection, ApiVersion};
    use zksync_types::{Address, BlockNumber, ZkSyncTx};

    async fn is_token_enabled_for_fees(
        storage: &mut StorageProcessor<'_>,
        token_id: TokenId,
        config: &ZkSyncConfig,
    ) -> anyhow::Result<bool> {
        let min_market_volume = Ratio::from(
            BigUint::from_f64(config.ticker.liquidity_volume)
                .expect("TickerConfig::liquidity_volume must be positive"),
        );
        let filtered = storage
            .tokens_schema()
            .filter_tokens_by_market_volume(vec![token_id], &min_market_volume)
            .await?;
        Ok(!filtered.is_empty())
    }

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn tokens_scope() -> anyhow::Result<()> {
        let cfg = TestServerConfig::default();
        cfg.fill_database().await?;

        let prices = vec![
            (TokenLike::Id(TokenId(1)), 10_u64.into()),
            (TokenLike::Symbol(String::from("PHNX")), 10_u64.into()),
            (TokenLike::Id(TokenId(15)), 10_500_u64.into()),
            (Address::default().into(), 1_u64.into()),
        ];

        let fee_ticker = dummy_fee_ticker(&prices, None);

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
            Some(shared_data),
        );

        let token_like = TokenLike::Id(TokenId(1));
        let response = client.token_by_id(&token_like).await?;
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
            from: ApiEither::from(TokenId(15)),
            limit: 2,
            direction: PaginationDirection::Older,
        };
        let response = client.token_pagination(&query).await?;
        let pagination: Paginated<ApiToken, TokenId> = deserialize_response_result(response)?;

        let expected_pagination = {
            let mut storage = cfg.pool.access_storage().await?;
            let paginated_tokens: Paginated<Token, TokenId> = storage
                .paginate_checked(&query)
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
            Paginated::new(
                list,
                paginated_tokens.pagination.from,
                paginated_tokens.pagination.limit,
                paginated_tokens.pagination.direction,
                paginated_tokens.pagination.count,
            )
        };
        assert_eq!(pagination, expected_pagination);

        let token_like = TokenLike::Symbol(String::from("PHNX"));
        let token = {
            let mut storage = cfg.pool.access_storage().await?;
            storage
                .tokens_schema()
                .get_token(token_like.clone())
                .await?
                .unwrap()
        };
        let mut expected_token_price = TokenPrice {
            token_id: token.id,
            token_symbol: token.symbol,
            price_in: String::from("15"),
            decimals: token.decimals,
            price: BigDecimal::from_u32(10).unwrap() / BigDecimal::from_u32(10500).unwrap(),
        };

        let response = client.token_price(&token_like, "15").await?;
        let price_in_token: TokenPrice = deserialize_response_result(response)?;
        assert_eq!(price_in_token, expected_token_price);

        expected_token_price.price_in = String::from("usd");
        expected_token_price.price = BigDecimal::from_u32(10).unwrap();

        let response = client.token_price(&token_like, "usd").await?;
        let price_in_usd: TokenPrice = deserialize_response_result(response)?;
        assert_eq!(price_in_usd, expected_token_price);

        let response = client.token_price(&token_like, "333").await?;
        assert!(response.error.is_some());

        let nft_id = TokenId(65542);
        let response = client.nft_by_id(nft_id).await?;
        let nft: ApiNFT = deserialize_response_result(response)?;
        assert_eq!(nft.id, nft_id);

        let response = client.nft_owner_by_id(nft_id).await?;
        let owner_id: AccountId = deserialize_response_result(response)?;
        let expected_owner_id = {
            let mut storage = cfg.pool.access_storage().await?;
            storage
                .chain()
                .account_schema()
                .get_nft_owner(nft_id)
                .await?
                .unwrap()
        };
        assert_eq!(owner_id, expected_owner_id);

        let mut block_number = BlockNumber(0);
        let tx_hash = loop {
            let mut storage = cfg.pool.access_storage().await?;
            let ops = storage
                .chain()
                .block_schema()
                .get_block_executed_ops(block_number)
                .await?;
            let mut tx_hash: Option<TxHash> = None;
            for op in ops {
                if let Some(tx) = op.get_executed_tx() {
                    if matches!(tx.signed_tx.tx, ZkSyncTx::MintNFT(_)) {
                        tx_hash = Some(tx.signed_tx.tx.hash());
                    }
                }
            }
            if let Some(tx_hash) = tx_hash {
                break tx_hash;
            }
            block_number.0 += 1;
        };

        let response = client.nft_id_by_tx_hash(tx_hash).await?;
        let nft_id: Option<TokenId> = deserialize_response_result(response)?;
        assert!(nft_id.is_some());

        server.stop().await;
        Ok(())
    }
}
