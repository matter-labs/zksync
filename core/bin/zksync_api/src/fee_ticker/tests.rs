use super::*;
use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::Utc;
use futures::channel::mpsc;
use futures::executor::block_on;
use std::str::FromStr;
use zksync_types::{Address, Token, TokenId, TokenPrice};
use zksync_utils::{ratio_to_big_decimal, UnsignedRatioSerializeAsDecimal};

const TEST_FAST_WITHDRAW_COEFF: u64 = 10;

#[derive(Debug, Clone)]
struct TestToken {
    id: TokenId,
    price_usd: Ratio<BigUint>,
    risk_factor: Option<Ratio<BigUint>>,
    precision: u8,
}

impl TestToken {
    fn new(id: TokenId, price_usd: f64, risk_factor: Option<f64>, precision: u8) -> Self {
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
        }
    }

    fn risk_factor(&self) -> Ratio<BigUint> {
        self.risk_factor
            .clone()
            .unwrap_or_else(|| Ratio::from_integer(1u32.into()))
    }

    fn eth() -> Self {
        Self::new(0, 182.0, None, 18)
    }

    fn cheap() -> Self {
        Self::new(1, 1.0, Some(2.5), 6)
    }
    fn expensive() -> Self {
        Self::new(2, 173_134.192_3, Some(0.9), 18)
    }

    fn all_tokens() -> Vec<Self> {
        vec![Self::eth(), Self::cheap(), Self::expensive()]
    }
}

fn get_test_ticker_config() -> TickerConfig {
    TickerConfig {
        zkp_cost_chunk_usd: UnsignedRatioSerializeAsDecimal::deserialize_from_str_with_dot("0.001")
            .unwrap(),
        gas_cost_tx: vec![
            (
                OutputFeeType::Transfer,
                BigUint::from(constants::BASE_TRANSFER_COST),
            ),
            (
                OutputFeeType::TransferToNew,
                BigUint::from(constants::BASE_TRANSFER_TO_NEW_COST),
            ),
            (
                OutputFeeType::Withdraw,
                BigUint::from(constants::BASE_WITHDRAW_COST),
            ),
            (
                OutputFeeType::FastWithdraw,
                BigUint::from(constants::BASE_WITHDRAW_COST * TEST_FAST_WITHDRAW_COEFF),
            ),
            (
                OutputFeeType::ChangePubKey {
                    onchain_pubkey_auth: false,
                },
                constants::BASE_CHANGE_PUBKEY_OFFCHAIN_COST.into(),
            ),
            (
                OutputFeeType::ChangePubKey {
                    onchain_pubkey_auth: true,
                },
                constants::BASE_CHANGE_PUBKEY_ONCHAIN_COST.into(),
            ),
        ]
        .into_iter()
        .collect(),
        tokens_risk_factors: TestToken::all_tokens()
            .into_iter()
            .filter_map(|t| {
                let id = t.id;
                t.risk_factor.map(|risk| (id, risk))
            })
            .collect(),
    }
}

struct MockApiProvider;
#[async_trait]
impl FeeTickerAPI for MockApiProvider {
    async fn get_last_quote(&self, token: TokenLike) -> Result<TokenPrice, anyhow::Error> {
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
                    Address::default(),
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

#[test]
fn test_ticker_formula() {
    let validator = FeeTokenValidator::new(HashMap::new());

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

            // Fee in usd
            (block_on(MockApiProvider.get_last_quote(token))
                .expect("failed to get fee in usd")
                .usd_price
                / BigUint::from(10u32).pow(u32::from(token_precision)))
                * fee_in_token.total_fee
        };

    let get_relative_diff = |a: &Ratio<BigUint>, b: &Ratio<BigUint>| -> BigDecimal {
        let max = std::cmp::max(a.clone(), b.clone());
        let min = std::cmp::min(a.clone(), b.clone());
        ratio_to_big_decimal(&((&max - &min) / min), 6)
    };

    {
        let expected_price_of_eth_token_transfer_usd =
            get_token_fee_in_usd(TxFeeTypes::Transfer, 0.into(), Address::default());
        let expected_price_of_eth_token_withdraw_usd =
            get_token_fee_in_usd(TxFeeTypes::Withdraw, 0.into(), Address::default());
        let expected_price_of_eth_token_fast_withdraw_usd =
            get_token_fee_in_usd(TxFeeTypes::FastWithdraw, 0.into(), Address::default());

        // Cost of the transfer and withdraw in USD should be the same for all tokens up to +/- 3 digits (mantissa len == 11)
        let threshold = BigDecimal::from_str("0.01").unwrap();
        for token in TestToken::all_tokens() {
            let transfer_fee =
                get_token_fee_in_usd(TxFeeTypes::Transfer, token.id.into(), Address::default());
            let expected_fee =
                expected_price_of_eth_token_transfer_usd.clone() * token.risk_factor();
            let transfer_diff = get_relative_diff(&transfer_fee, &expected_fee);
            assert!(
                    transfer_diff <= threshold.clone(),
                    "token transfer fee is above eth fee threshold: <{:?}: {}, ETH: {}, diff: {}, threshold: {}>", token.id, 
                    UnsignedRatioSerializeAsDecimal::serialize_to_str_with_dot(&transfer_fee,6),
                    UnsignedRatioSerializeAsDecimal::serialize_to_str_with_dot(&expected_fee,6),
                    transfer_diff, &threshold);

            let withdraw_fee =
                get_token_fee_in_usd(TxFeeTypes::Withdraw, token.id.into(), Address::default());
            let expected_fee =
                expected_price_of_eth_token_withdraw_usd.clone() * token.risk_factor();
            let withdraw_diff = get_relative_diff(&withdraw_fee, &expected_fee);
            assert!(
                    withdraw_diff <= threshold.clone(),
                    "token withdraw fee is above eth fee threshold: <{:?}: {}, ETH: {}, diff: {}, threshold: {}>", token.id,
                    UnsignedRatioSerializeAsDecimal::serialize_to_str_with_dot(&withdraw_fee,6),
                    UnsignedRatioSerializeAsDecimal::serialize_to_str_with_dot(&expected_fee,6),
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
                    "token fast withdraw fee is above eth fee threshold: <{:?}: {}, ETH: {}, diff: {}, threshold: {}>", token.id,
                    UnsignedRatioSerializeAsDecimal::serialize_to_str_with_dot(&fast_withdraw_fee,6),
                    UnsignedRatioSerializeAsDecimal::serialize_to_str_with_dot(&expected_fee,6),
                    fast_withdraw_diff, &threshold
                );
            assert!(
                fast_withdraw_fee > withdraw_fee,
                "Fast withdraw fee must be greater than usual withdraw fee"
            );
        }
    }
}
