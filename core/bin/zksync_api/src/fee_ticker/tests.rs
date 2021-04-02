use actix_web::{web, App, HttpResponse, HttpServer};

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::Utc;
use futures::future::{AbortHandle, Abortable};
use futures::{channel::mpsc, executor::block_on};
use std::str::FromStr;
use std::thread::sleep;
use tokio::time::Duration;
use zksync_types::{Address, Token, TokenId, TokenPrice};
use zksync_utils::{big_decimal_to_ratio, ratio_to_big_decimal, UnsignedRatioSerializeAsDecimal};

use crate::fee_ticker::{
    ticker_api::{
        coingecko::{CoinGeckoTokenInfo, CoinGeckoTokenList},
        TokenPriceAPI,
    },
    validator::{cache::TokenInMemoryCache, FeeTokenValidator},
};

use super::*;

const TEST_FAST_WITHDRAW_COEFF: f64 = 10.0;

#[derive(Debug, Clone)]
struct TestToken {
    id: TokenId,
    price_usd: Ratio<BigUint>,
    risk_factor: Option<Ratio<BigUint>>,
    precision: u8,
    address: Address,
}

impl TestToken {
    fn new(
        id: TokenId,
        price_usd: f64,
        risk_factor: Option<f64>,
        precision: u8,
        address: Address,
    ) -> Self {
        Self {
            id,
            price_usd: UnsignedRatioSerializeAsDecimal::deserialize_from_str_with_dot(
                &price_usd.to_string(),
            )
            .unwrap(),
            risk_factor: risk_factor.map(|risk_factor| {
                UnsignedRatioSerializeAsDecimal::deserialize_from_str_with_dot(
                    &risk_factor.to_string(),
                )
                .unwrap()
            }),
            precision,
            address,
        }
    }

    fn risk_factor(&self) -> Ratio<BigUint> {
        self.risk_factor
            .clone()
            .unwrap_or_else(|| Ratio::from_integer(1u32.into()))
    }

    fn eth() -> Self {
        Self::new(TokenId(0), 182.0, None, 18, Address::default())
    }

    fn hex() -> Self {
        Self::new(
            TokenId(1),
            1.0,
            Some(2.5),
            6,
            Address::from_str("34083bbd70d394110487feaa087da875a54624ec").unwrap(),
        )
    }

    fn cheap() -> Self {
        Self::new(TokenId(2), 1.0, Some(2.5), 6, Address::default())
    }

    fn expensive() -> Self {
        Self::new(TokenId(3), 173_134.192_3, Some(0.9), 18, Address::default())
    }

    fn zero_price() -> Self {
        Self::new(TokenId(4), 0.0, Some(0.9), 18, Address::default())
    }

    fn subsidized_tokens() -> Vec<Self> {
        vec![Self::eth(), Self::cheap(), Self::expensive()]
    }

    fn unsubsidized_tokens() -> Vec<Self> {
        vec![Self::hex(), Self::zero_price()]
    }

    fn all_tokens() -> Vec<Self> {
        let mut all_tokens = Vec::new();
        all_tokens.extend_from_slice(&Self::subsidized_tokens());
        all_tokens.extend_from_slice(&Self::unsubsidized_tokens());

        all_tokens
    }
}

fn get_test_ticker_config() -> TickerConfig {
    TickerConfig {
        zkp_cost_chunk_usd: UnsignedRatioSerializeAsDecimal::deserialize_from_str_with_dot("0.001")
            .unwrap(),
        gas_cost_tx: GasOperationsCost::from_constants(TEST_FAST_WITHDRAW_COEFF),
        tokens_risk_factors: TestToken::all_tokens()
            .into_iter()
            .filter_map(|t| {
                let id = t.id;
                t.risk_factor.map(|risk| (id, risk))
            })
            .collect(),
        not_subsidized_tokens: vec![
            Address::from_str("34083bbd70d394110487feaa087da875a54624ec").unwrap(),
        ]
        .into_iter()
        .collect(),
    }
}

struct MockApiProvider;
#[async_trait]
impl FeeTickerAPI for MockApiProvider {
    async fn get_last_quote(&self, token: TokenLike) -> Result<TokenPrice, PriceError> {
        for test_token in TestToken::all_tokens() {
            if TokenLike::Id(test_token.id) == token {
                let token_price = TokenPrice {
                    usd_price: test_token.price_usd,
                    last_updated: Utc::now(),
                };
                return Ok(token_price);
            }
        }
        unreachable!("incorrect token input")
    }

    /// Get current gas price in ETH
    async fn get_gas_price_wei(&self) -> Result<BigUint, anyhow::Error> {
        Ok(BigUint::from(10u32).pow(7u32)) // 10 GWei
    }

    async fn get_token(&self, token: TokenLike) -> Result<Token, anyhow::Error> {
        for test_token in TestToken::all_tokens() {
            if TokenLike::Id(test_token.id) == token {
                return Ok(Token::new(
                    test_token.id,
                    test_token.address,
                    "",
                    test_token.precision,
                ));
            }
        }
        unreachable!("incorrect token input")
    }
}

struct MockTickerInfo;

#[async_trait]
impl FeeTickerInfo for MockTickerInfo {
    async fn is_account_new(&mut self, _address: Address) -> bool {
        // Always false for simplicity.
        false
    }
}

fn format_with_dot(num: &Ratio<BigUint>, precision: usize) -> String {
    UnsignedRatioSerializeAsDecimal::serialize_to_str_with_dot(num, precision)
}

#[derive(Debug)]
struct FakeTokenWatcher;

#[async_trait::async_trait]
impl TokenWatcher for FakeTokenWatcher {
    async fn get_token_market_volume(&mut self, _token: &Token) -> anyhow::Result<BigDecimal> {
        unreachable!()
    }
}

struct ErrorTickerApi;

#[async_trait::async_trait]
impl TokenPriceAPI for ErrorTickerApi {
    async fn get_price(&self, _token_symbol: &str) -> Result<TokenPrice, PriceError> {
        Err(PriceError::token_not_found("Wrong token"))
    }
}

fn run_server() -> (String, AbortHandle) {
    let mut url = None;
    let mut server = None;
    for i in 9000..9999 {
        let new_url = format!("127.0.0.1:{}", i);
        // Try to bind to some port, hope that 999 variants will be enough
        if let Ok(ser) = HttpServer::new(move || {
            App::new()
                .service(
                    web::resource("/api/v3/coins/DAI/market_chart").route(web::get().to(|| {
                        sleep(Duration::from_secs(100));
                        HttpResponse::MethodNotAllowed()
                    })),
                )
                .service(web::resource("/api/v3/coins/list").to(|| {
                    HttpResponse::Ok().json(CoinGeckoTokenList(vec![CoinGeckoTokenInfo {
                        id: "DAI".to_string(),
                        symbol: "DAI".to_string(),
                    }]))
                }))
        })
        .bind(new_url.clone())
        {
            server = Some(ser);
            url = Some(new_url);
            break;
        }
    }

    let server = server.expect("Could not bind to port from 9000 to 9999");
    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    let future = Abortable::new(server.run(), abort_registration);
    tokio::spawn(future);
    let address = format!("http://{}/", &url.unwrap());
    (address, abort_handle)
}

#[test]
fn test_ticker_formula() {
    let validator = FeeTokenValidator::new(
        TokenInMemoryCache::new(),
        chrono::Duration::seconds(100),
        BigDecimal::from(100),
        Default::default(),
        FakeTokenWatcher,
    );

    let config = get_test_ticker_config();
    let mut ticker = FeeTicker::new(
        MockApiProvider,
        MockTickerInfo,
        mpsc::channel(1).1,
        config,
        validator,
    );

    let mut get_token_fee_in_usd =
        |tx_type: TxFeeTypes, token: TokenLike, address: Address| -> Ratio<BigUint> {
            let fee_in_token =
                block_on(ticker.get_fee_from_ticker_in_wei(tx_type, token.clone(), address))
                    .expect("failed to get fee in token");
            let token_precision = block_on(MockApiProvider.get_token(token.clone()))
                .unwrap()
                .decimals;
            let batched_fee_in_token = block_on(
                ticker.get_batch_from_ticker_in_wei(token.clone(), vec![(tx_type, address)]),
            )
            .expect("failed to get batched fee for token");
            assert_eq!(
                fee_in_token.normal_fee.total_fee,
                batched_fee_in_token.normal_fee.total_fee
            );

            // Fee in usd
            (block_on(MockApiProvider.get_last_quote(token))
                .expect("failed to get fee in usd")
                .usd_price
                / BigUint::from(10u32).pow(u32::from(token_precision)))
                * fee_in_token.normal_fee.total_fee
        };

    let get_relative_diff = |a: &Ratio<BigUint>, b: &Ratio<BigUint>| -> BigDecimal {
        let max = std::cmp::max(a.clone(), b.clone());
        let min = std::cmp::min(a.clone(), b.clone());
        ratio_to_big_decimal(&((&max - &min) / min), 6)
    };

    let expected_price_of_eth_token_transfer_usd =
        get_token_fee_in_usd(TxFeeTypes::Transfer, TokenId(0).into(), Address::default());
    let expected_price_of_eth_token_withdraw_usd =
        get_token_fee_in_usd(TxFeeTypes::Withdraw, TokenId(0).into(), Address::default());
    let expected_price_of_eth_token_fast_withdraw_usd = get_token_fee_in_usd(
        TxFeeTypes::FastWithdraw,
        TokenId(0).into(),
        Address::default(),
    );

    // Cost of the transfer and withdraw in USD should be the same for all tokens up to +/- 3 digits
    // (mantissa len == 11)
    let threshold = BigDecimal::from_str("0.01").unwrap();
    for token in TestToken::subsidized_tokens() {
        let transfer_fee =
            get_token_fee_in_usd(TxFeeTypes::Transfer, token.id.into(), Address::default());
        let expected_fee = expected_price_of_eth_token_transfer_usd.clone() * token.risk_factor();
        let transfer_diff = get_relative_diff(&transfer_fee, &expected_fee);
        assert!(
                transfer_diff <= threshold.clone(),
                "token transfer fee is above eth fee threshold: <{:?}: {}, ETH: {}, diff: {}, threshold: {}>",
                token.id,
                format_with_dot(&transfer_fee, 6),
                format_with_dot(&expected_fee, 6),
                transfer_diff, &threshold
            );

        let withdraw_fee =
            get_token_fee_in_usd(TxFeeTypes::Withdraw, token.id.into(), Address::default());
        let expected_fee = expected_price_of_eth_token_withdraw_usd.clone() * token.risk_factor();
        let withdraw_diff = get_relative_diff(&withdraw_fee, &expected_fee);
        assert!(
                withdraw_diff <= threshold.clone(),
                "token withdraw fee is above eth fee threshold: <{:?}: {}, ETH: {}, diff: {}, threshold: {}>",
                token.id,
                format_with_dot(&withdraw_fee, 6),
                format_with_dot(&expected_fee, 6),
                withdraw_diff, &threshold
            );

        let fast_withdraw_fee = get_token_fee_in_usd(
            TxFeeTypes::FastWithdraw,
            token.id.into(),
            Address::default(),
        );
        let expected_fee =
            expected_price_of_eth_token_fast_withdraw_usd.clone() * token.risk_factor();
        let fast_withdraw_diff = get_relative_diff(&fast_withdraw_fee, &expected_fee);
        assert!(
                fast_withdraw_diff <= threshold.clone(),
                "token fast withdraw fee is above eth fee threshold: <{:?}: {}, ETH: {}, diff: {}, threshold: {}>",
                token.id,
                format_with_dot(&fast_withdraw_fee, 6),
                format_with_dot(&expected_fee, 6),
                fast_withdraw_diff, &threshold
            );
        assert!(
            fast_withdraw_fee > withdraw_fee,
            "Fast withdraw fee must be greater than usual withdraw fee"
        );
    }
}

// It's temporary solution while zero-price tokens marked as allowed for fee
#[test]
fn test_zero_price_token_fee() {
    let validator = FeeTokenValidator::new(
        TokenInMemoryCache::new(),
        chrono::Duration::seconds(100),
        BigDecimal::from(100),
        Default::default(),
        FakeTokenWatcher,
    );

    let config = get_test_ticker_config();
    let mut ticker = FeeTicker::new(
        MockApiProvider,
        MockTickerInfo,
        mpsc::channel(1).1,
        config,
        validator,
    );

    let token = TestToken::zero_price();

    // If token allowed for fee and price is zero, it should return error
    block_on(ticker.get_fee_from_ticker_in_wei(
        TxFeeTypes::Transfer,
        token.id.into(),
        Address::default(),
    ))
    .unwrap_err();

    block_on(ticker.get_batch_from_ticker_in_wei(
        token.id.into(),
        vec![(TxFeeTypes::Transfer, Address::default())],
    ))
    .unwrap_err();
}

#[actix_rt::test]
#[ignore]
// It's ignore because we can't initialize coingecko in current way with block
async fn test_error_coingecko_api() {
    let (address, handler) = run_server();
    let client = reqwest::ClientBuilder::new()
        .timeout(CONNECTION_TIMEOUT)
        .connect_timeout(CONNECTION_TIMEOUT)
        .build()
        .expect("Failed to build reqwest::Client");
    let coingecko = CoinGeckoAPI::new(client, address.parse().unwrap()).unwrap();
    let validator = FeeTokenValidator::new(
        TokenInMemoryCache::new(),
        chrono::Duration::seconds(100),
        BigDecimal::from(100),
        Default::default(),
        FakeTokenWatcher,
    );
    let connection_pool = ConnectionPool::new(Some(1));
    connection_pool
        .access_storage()
        .await
        .unwrap()
        .tokens_schema()
        .update_historical_ticker_price(
            TokenId(1),
            TokenPrice {
                usd_price: big_decimal_to_ratio(&BigDecimal::from(10)).unwrap(),
                last_updated: chrono::offset::Utc::now(),
            },
        )
        .await
        .unwrap();
    let ticker_api = TickerApi::new(connection_pool, coingecko);

    let config = get_test_ticker_config();
    let mut ticker = FeeTicker::new(
        ticker_api,
        MockTickerInfo,
        mpsc::channel(1).1,
        config,
        validator,
    );
    for _ in 0..1000 {
        ticker
            .get_fee_from_ticker_in_wei(
                TxFeeTypes::FastWithdraw,
                TokenId(1).into(),
                Address::default(),
            )
            .await
            .unwrap();
        ticker
            .get_token_price(TokenId(1).into(), TokenPriceRequestType::USDForOneWei)
            .await
            .unwrap();
    }
    handler.abort();
}

#[tokio::test]
#[ignore]
async fn test_error_api() {
    let validator = FeeTokenValidator::new(
        TokenInMemoryCache::new(),
        chrono::Duration::seconds(100),
        BigDecimal::from(100),
        Default::default(),
        FakeTokenWatcher,
    );
    let connection_pool = ConnectionPool::new(Some(1));
    let second_connection_pool = connection_pool.clone();
    let ticker_api = TickerApi::new(second_connection_pool, ErrorTickerApi);
    connection_pool
        .access_storage()
        .await
        .unwrap()
        .tokens_schema()
        .update_historical_ticker_price(
            TokenId(1),
            TokenPrice {
                usd_price: big_decimal_to_ratio(&BigDecimal::from(10)).unwrap(),
                last_updated: chrono::offset::Utc::now(),
            },
        )
        .await
        .unwrap();
    let config = get_test_ticker_config();
    let mut ticker = FeeTicker::new(
        ticker_api,
        MockTickerInfo,
        mpsc::channel(1).1,
        config,
        validator,
    );

    ticker
        .get_fee_from_ticker_in_wei(
            TxFeeTypes::FastWithdraw,
            TokenId(1).into(),
            Address::default(),
        )
        .await
        .unwrap();
    ticker
        .get_token_price(TokenId(1).into(), TokenPriceRequestType::USDForOneWei)
        .await
        .unwrap();
}
