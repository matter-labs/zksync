//! Module used to calculate fee for transactions.
//!
//! base formula for calculation:
//! `( zkp cost of chunk * number of chunks + gas price of transaction) * token risk factor / cost of token is usd`

// Built-in deps
use std::collections::HashMap;
// External deps
use bigdecimal::BigDecimal;
use futures::{
    channel::{mpsc::Receiver, oneshot},
    StreamExt,
};
use num::{
    rational::Ratio,
    traits::{Inv, Pow},
    BigUint,
};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;
// Workspace deps
use zksync_config::TokenPriceSource;
use zksync_storage::ConnectionPool;
use zksync_types::{
    gas_counter::{CommitCost, GasCounter, VerifyCost},
    helpers::{pack_fee_amount, unpack_fee_amount},
    Address, ChangePubKeyOp, TokenId, TokenLike, TransferOp, TransferToNewOp, TxFeeTypes,
    WithdrawOp,
};
use zksync_utils::{ratio_to_big_decimal, round_precision, BigUintSerdeAsRadix10Str};
// Local deps
use crate::fee_ticker::ticker_api::coingecko::CoinGeckoAPI;
use crate::fee_ticker::ticker_api::coinmarkercap::CoinMarketCapAPI;
use crate::fee_ticker::{
    ticker_api::{FeeTickerAPI, TickerApi, CONNECTION_TIMEOUT},
    ticker_info::{FeeTickerInfo, TickerInfo},
};
use zksync_types::config::MAX_WITHDRAWALS_TO_COMPLETE_IN_A_CALL;

mod ticker_api;
mod ticker_info;

// Base operation costs estimated via `gas_price` test.
//
// Factor of 1000 * CHUNKS accounts for constant overhead of the commit and verify for block of 680 chunks
// (140k + 530k) / 680. Should be removed after recursion is introduced to mainnet.
const BASE_TRANSFER_COST: u64 =
    VerifyCost::TRANSFER_COST + CommitCost::TRANSFER_COST + 1000 * (TransferOp::CHUNKS as u64);
const BASE_TRANSFER_TO_NEW_COST: u64 = VerifyCost::TRANSFER_TO_NEW_COST
    + CommitCost::TRANSFER_TO_NEW_COST
    + 1000 * (TransferToNewOp::CHUNKS as u64);
const BASE_WITHDRAW_COST: u64 = VerifyCost::WITHDRAW_COST
    + CommitCost::WITHDRAW_COST
    + GasCounter::COMPLETE_WITHDRAWALS_COST
    + 1000 * (WithdrawOp::CHUNKS as u64)
    + (GasCounter::COMPLETE_WITHDRAWALS_BASE_COST / MAX_WITHDRAWALS_TO_COMPLETE_IN_A_CALL);
const BASE_CHANGE_PUBKEY_OFFCHAIN_COST: u64 = CommitCost::CHANGE_PUBKEY_COST_OFFCHAIN
    + VerifyCost::CHANGE_PUBKEY_COST
    + 1000 * (ChangePubKeyOp::CHUNKS as u64);
const BASE_CHANGE_PUBKEY_ONCHAIN_COST: u64 = CommitCost::CHANGE_PUBKEY_COST_ONCHAIN
    + zksync_types::gas_counter::VerifyCost::CHANGE_PUBKEY_COST
    + 1000 * (ChangePubKeyOp::CHUNKS as u64);

/// Type of the fee calculation pattern.
/// Unlike the `TxFeeTypes`, this enum represents the fee
/// from the point of zkSync view, rather than from the users
/// point of view.
/// Users do not divide transfers into `Transfer` and
/// `TransferToNew`, while in zkSync it's two different operations.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutputFeeType {
    Transfer,
    TransferToNew,
    Withdraw,
    FastWithdraw,
    ChangePubKey { onchain_pubkey_auth: bool },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Fee {
    pub fee_type: OutputFeeType,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub gas_tx_amount: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub gas_price_wei: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub gas_fee: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub zkp_fee: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub total_fee: BigUint,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BatchFee {
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub total_fee: BigUint,
}

impl Fee {
    pub fn new(
        fee_type: OutputFeeType,
        zkp_fee: Ratio<BigUint>,
        gas_fee: Ratio<BigUint>,
        gas_tx_amount: BigUint,
        gas_price_wei: BigUint,
    ) -> Self {
        let zkp_fee = round_precision(&zkp_fee, 18).ceil().to_integer();
        let gas_fee = round_precision(&gas_fee, 18).ceil().to_integer();

        let total_fee = zkp_fee.clone() + gas_fee.clone();
        let total_fee = unpack_fee_amount(&pack_fee_amount(&total_fee))
            .expect("Failed to round gas fee amount.");

        Self {
            fee_type,
            gas_tx_amount,
            gas_price_wei,
            gas_fee,
            zkp_fee,
            total_fee,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TickerConfig {
    zkp_cost_chunk_usd: Ratio<BigUint>,
    gas_cost_tx: HashMap<OutputFeeType, BigUint>, //wei
    tokens_risk_factors: HashMap<TokenId, Ratio<BigUint>>,
}

pub enum TokenPriceRequestType {
    USDForOneWei,
    USDForOneToken,
}

pub enum TickerRequest {
    GetTxFee {
        tx_type: TxFeeTypes,
        address: Address,
        token: TokenLike,
        response: oneshot::Sender<Result<Fee, anyhow::Error>>,
    },
    GetTokenPrice {
        token: TokenLike,
        response: oneshot::Sender<Result<BigDecimal, anyhow::Error>>,
        req_type: TokenPriceRequestType,
    },
}

struct FeeTicker<API, INFO> {
    api: API,
    info: INFO,
    requests: Receiver<TickerRequest>,
    config: TickerConfig,
}

#[must_use]
pub fn run_ticker_task(
    token_price_source: TokenPriceSource,
    fast_processing_coeff: f64,
    db_pool: ConnectionPool,
    tricker_requests: Receiver<TickerRequest>,
) -> JoinHandle<()> {
    // We increase gas price for fast withdrawals, since it will induce generating a smaller block
    // size, resulting in us paying more gas than for bigger block.
    let fast_withdrawal_cost = (BASE_WITHDRAW_COST as f64 * fast_processing_coeff) as u32;

    let ticker_config = TickerConfig {
        zkp_cost_chunk_usd: Ratio::from_integer(BigUint::from(10u32).pow(3u32)).inv(),
        gas_cost_tx: vec![
            (OutputFeeType::Transfer, BASE_TRANSFER_COST.into()),
            (
                OutputFeeType::TransferToNew,
                BASE_TRANSFER_TO_NEW_COST.into(),
            ),
            (OutputFeeType::Withdraw, BASE_WITHDRAW_COST.into()),
            (OutputFeeType::FastWithdraw, fast_withdrawal_cost.into()),
            (
                OutputFeeType::ChangePubKey {
                    onchain_pubkey_auth: false,
                },
                BASE_CHANGE_PUBKEY_OFFCHAIN_COST.into(),
            ),
            (
                OutputFeeType::ChangePubKey {
                    onchain_pubkey_auth: true,
                },
                BASE_CHANGE_PUBKEY_ONCHAIN_COST.into(),
            ),
        ]
        .into_iter()
        .collect(),
        tokens_risk_factors: HashMap::new(),
    };

    let client = reqwest::ClientBuilder::new()
        .timeout(CONNECTION_TIMEOUT)
        .connect_timeout(CONNECTION_TIMEOUT)
        .build()
        .expect("Failed to build reqwest::Client");
    match token_price_source {
        TokenPriceSource::CoinMarketCap { base_url } => {
            let token_price_api = CoinMarketCapAPI::new(client, base_url);

            let ticker_api = TickerApi::new(db_pool.clone(), token_price_api);
            let ticker_info = TickerInfo::new(db_pool);
            let fee_ticker =
                FeeTicker::new(ticker_api, ticker_info, tricker_requests, ticker_config);

            tokio::spawn(fee_ticker.run())
        }
        TokenPriceSource::CoinGecko { base_url } => {
            let token_price_api =
                CoinGeckoAPI::new(client, base_url).expect("failed to init CoinGecko client");

            let ticker_api = TickerApi::new(db_pool.clone(), token_price_api);
            let ticker_info = TickerInfo::new(db_pool);
            let fee_ticker =
                FeeTicker::new(ticker_api, ticker_info, tricker_requests, ticker_config);

            tokio::spawn(fee_ticker.run())
        }
    }
}

impl<API: FeeTickerAPI, INFO: FeeTickerInfo> FeeTicker<API, INFO> {
    fn new(api: API, info: INFO, requests: Receiver<TickerRequest>, config: TickerConfig) -> Self {
        Self {
            api,
            info,
            requests,
            config,
        }
    }

    async fn run(mut self) {
        while let Some(request) = self.requests.next().await {
            match request {
                TickerRequest::GetTxFee {
                    tx_type,
                    token,
                    response,
                    address,
                } => {
                    let fee = self
                        .get_fee_from_ticker_in_wei(tx_type, token, address)
                        .await;
                    response.send(fee).unwrap_or_default();
                }
                TickerRequest::GetTokenPrice {
                    token,
                    response,
                    req_type,
                } => {
                    let price = self.get_token_price(token, req_type).await;
                    response.send(price).unwrap_or_default();
                }
            }
        }
    }

    async fn get_token_price(
        &self,
        token: TokenLike,
        req_rype: TokenPriceRequestType,
    ) -> Result<BigDecimal, anyhow::Error> {
        let factor = match req_rype {
            TokenPriceRequestType::USDForOneWei => {
                let token_decimals = self.api.get_token(token.clone()).await?.decimals;
                BigUint::from(10u32).pow(u32::from(token_decimals))
            }
            TokenPriceRequestType::USDForOneToken => BigUint::from(1u32),
        };

        self.api
            .get_last_quote(token)
            .await
            .map(|price| ratio_to_big_decimal(&(price.usd_price / factor), 100))
    }

    /// Returns `true` if account does not yet exist in the zkSync network.
    async fn is_account_new(&mut self, address: Address) -> bool {
        self.info.is_account_new(address).await
    }

    async fn get_fee_from_ticker_in_wei(
        &mut self,
        tx_type: TxFeeTypes,
        token: TokenLike,
        recipient: Address,
    ) -> Result<Fee, anyhow::Error> {
        let zkp_cost_chunk = self.config.zkp_cost_chunk_usd.clone();
        let token = self.api.get_token(token).await?;
        let token_risk_factor = self
            .config
            .tokens_risk_factors
            .get(&token.id)
            .cloned()
            .unwrap_or_else(|| Ratio::from_integer(1u32.into()));

        let (fee_type, op_chunks) = match tx_type {
            TxFeeTypes::Withdraw => (OutputFeeType::Withdraw, WithdrawOp::CHUNKS),
            TxFeeTypes::FastWithdraw => (OutputFeeType::FastWithdraw, WithdrawOp::CHUNKS),
            TxFeeTypes::Transfer => {
                if self.is_account_new(recipient).await {
                    (OutputFeeType::TransferToNew, TransferToNewOp::CHUNKS)
                } else {
                    (OutputFeeType::Transfer, TransferOp::CHUNKS)
                }
            }
            TxFeeTypes::ChangePubKey {
                onchain_pubkey_auth,
            } => (
                OutputFeeType::ChangePubKey {
                    onchain_pubkey_auth,
                },
                ChangePubKeyOp::CHUNKS,
            ),
        };
        // Convert chunks amount to `BigUint`.
        let op_chunks = BigUint::from(op_chunks);
        let gas_tx_amount = self.config.gas_cost_tx.get(&fee_type).cloned().unwrap();
        let gas_price_wei = self.api.get_gas_price_wei().await?;
        let wei_price_usd = self.api.get_last_quote(TokenLike::Id(0)).await?.usd_price
            / BigUint::from(10u32).pow(18u32);

        let token_price_usd = self
            .api
            .get_last_quote(TokenLike::Id(token.id))
            .await?
            .usd_price
            / BigUint::from(10u32).pow(u32::from(token.decimals));

        let zkp_fee =
            (zkp_cost_chunk * op_chunks) * token_risk_factor.clone() / token_price_usd.clone();
        let gas_fee = (wei_price_usd * gas_tx_amount.clone() * gas_price_wei.clone())
            * token_risk_factor
            / token_price_usd;

        Ok(Fee::new(
            fee_type,
            zkp_fee,
            gas_fee,
            gas_tx_amount,
            gas_price_wei,
        ))
    }
}

#[cfg(test)]
mod test {
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
            zkp_cost_chunk_usd: UnsignedRatioSerializeAsDecimal::deserialize_from_str_with_dot(
                "0.001",
            )
            .unwrap(),
            gas_cost_tx: vec![
                (OutputFeeType::Transfer, BigUint::from(BASE_TRANSFER_COST)),
                (
                    OutputFeeType::TransferToNew,
                    BigUint::from(BASE_TRANSFER_TO_NEW_COST),
                ),
                (OutputFeeType::Withdraw, BigUint::from(BASE_WITHDRAW_COST)),
                (
                    OutputFeeType::FastWithdraw,
                    BigUint::from(BASE_WITHDRAW_COST * TEST_FAST_WITHDRAW_COEFF),
                ),
                (
                    OutputFeeType::ChangePubKey {
                        onchain_pubkey_auth: false,
                    },
                    BASE_CHANGE_PUBKEY_OFFCHAIN_COST.into(),
                ),
                (
                    OutputFeeType::ChangePubKey {
                        onchain_pubkey_auth: true,
                    },
                    BASE_CHANGE_PUBKEY_ONCHAIN_COST.into(),
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
        let config = get_test_ticker_config();
        let mut ticker =
            FeeTicker::new(MockApiProvider, MockTickerInfo, mpsc::channel(1).1, config);

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
}
