use crate::fee_ticker::ticker_api::UnsignedRationalUtils;
use chrono::{DateTime, Utc};
use futures::channel::mpsc::{self, Receiver};
use models::node::{
    closest_packable_fee_amount, is_fee_amount_packable, TokenLike, TransferOp, WithdrawOp,
};
use models::params::{FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH};
use num::bigint::ToBigUint;
use num::rational::Ratio;
use num::traits::{Inv, Pow};
use num::{BigInt, BigUint};
use std::collections::HashMap;
use std::str::FromStr;
use storage::ConnectionPool;
use ticker_api::FeeTickerAPI;
use tokio::runtime::Runtime;

mod ticker_api;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TickerConfig {
    zkp_cost_chunk_usd: Ratio<BigUint>,
    gas_cost_tx: HashMap<TxFeeTypes, BigUint>, //wei
    tokens_risk_factors: HashMap<TokenLike, Ratio<BigUint>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Hash, Eq)]
pub enum TxFeeTypes {
    Withdraw,
    Transfer,
}

pub enum TickerRequest {
    GetTxFee {
        tx_type: TxFeeTypes,
        amount: BigUint,
        token: TokenLike,
    },
}

struct FeeTicker<API> {
    api: API,
    requests: Receiver<TickerRequest>,
    config: TickerConfig,
}

pub fn run_committer(ricker_requests: Receiver<TickerRequest>, runtime: &Runtime) {
    // let ticker_config = TickerConfig::default();
    // let FeeTicker::new()

    // runtime.spawn(handle_new_commit_task(
    //     rx_for_ops,
    //     tx_for_eth.clone(),
    //     op_notify_sender,
    //     mempool_req_sender,
    //     pool.clone(),
    // ));
    // runtime.spawn(poll_for_new_proofs_task(tx_for_eth, pool));
}

impl<API: FeeTickerAPI> FeeTicker<API> {
    fn new(api: API, requests: Receiver<TickerRequest>, config: TickerConfig) -> Self {
        Self {
            api,
            requests,
            config,
        }
    }

    async fn get_fee_from_ticker_in_wei_rounded(
        &self,
        tx_type: TxFeeTypes,
        token: TokenLike,
    ) -> Result<BigUint, failure::Error> {
        let ratio = self
            .get_fee_from_ticker_in_wei_exact(tx_type, token)
            .await?;

        let max_mantissa = BigUint::from(2u32).pow(FEE_MANTISSA_BIT_WIDTH);

        let mut rounded = UnsignedRationalUtils::round_precision(&ratio, 18)
            .ceil()
            .to_integer();

        let mut cum_remainder = BigUint::from(0u32);
        let mut current_power_mul = BigUint::from(1u32);
        while &rounded > &max_mantissa {
            let remainder = &rounded % 10u32;
            cum_remainder += &remainder * &current_power_mul;
            rounded -= &remainder;
            rounded /= 10u32;
            current_power_mul *= 10u32;
        }

        let round_ratio = Ratio::new(cum_remainder, current_power_mul.clone());
        if round_ratio >= Ratio::new(1u32.into(), 2u32.into()) && &rounded != &max_mantissa {
            rounded += 1u32;
        }
        let res = rounded * &current_power_mul;

        Ok(res)
    }

    async fn get_fee_from_ticker_in_wei_exact(
        &self,
        tx_type: TxFeeTypes,
        token: TokenLike,
    ) -> Result<Ratio<BigUint>, failure::Error> {
        let zkp_cost_chunk = self.config.zkp_cost_chunk_usd.clone();
        let token_risk_factor = self
            .config
            .tokens_risk_factors
            .get(&token)
            .cloned()
            .unwrap_or_else(|| Ratio::from_integer(1u32.into()));

        let op_chunks = BigUint::from(match tx_type {
            TxFeeTypes::Withdraw => WithdrawOp::CHUNKS,
            TxFeeTypes::Transfer => TransferOp::CHUNKS,
        });
        let gas_cost_tx = self.config.gas_cost_tx.get(&tx_type).cloned().unwrap();
        let gas_price_wei = self.api.get_gas_price_gwei().await? * BigUint::from(10u32).pow(6u32);
        let wei_price_usd = self.api.get_last_quote(TokenLike::Id(0)).await?.usd_price
            / BigUint::from(10u32).pow(18u32);
        let token_price_usd = self.api.get_last_quote(token.clone()).await?.usd_price
            / BigUint::from(10u32).pow(18u32); // TODO: add token precision here

        Ok(
            ((zkp_cost_chunk * op_chunks + wei_price_usd * gas_cost_tx * gas_price_wei)
                * token_risk_factor)
                / token_price_usd,
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fee_ticker::ticker_api::{TokenPrice, UnsignedRationalUtils};
    use crate::state_keeper::ExecutedOpId::Transaction;
    use futures::executor::block_on;
    use futures::Future;
    use models::node::block::ExecutedOperations::Tx;
    use models::node::{is_fee_amount_packable, TokenId};

    #[derive(Debug, Clone)]
    struct TestToken {
        id: TokenLike,
        price_usd: Ratio<BigUint>,
        risk_factor: Option<Ratio<BigUint>>,
    }

    impl TestToken {
        fn new(id: TokenId, price_usd: f64, risk_factor: Option<f64>) -> Self {
            Self {
                id: TokenLike::Id(id),
                price_usd: UnsignedRationalUtils::deserialize_for_str_with_dot(
                    &price_usd.to_string(),
                )
                .unwrap(),
                risk_factor: risk_factor.map(|risk_factor| {
                    UnsignedRationalUtils::deserialize_for_str_with_dot(&risk_factor.to_string())
                        .unwrap()
                }),
            }
        }

        fn risk_factor(&self) -> Ratio<BigUint> {
            self.risk_factor
                .clone()
                .unwrap_or_else(|| Ratio::from_integer(1u32.into()))
        }

        fn eth() -> Self {
            Self::new(0, 182.0, None)
        }

        fn cheap() -> Self {
            Self::new(1, 0.0016789, Some(2.5))
        }
        fn expensive() -> Self {
            Self::new(2, 173_134.1923, Some(0.9))
        }

        fn all_tokens() -> Vec<Self> {
            vec![Self::eth(), Self::cheap(), Self::expensive()]
        }
    }

    fn get_test_ticker_config() -> TickerConfig {
        TickerConfig {
            zkp_cost_chunk_usd: UnsignedRationalUtils::deserialize_for_str_with_dot("0.001")
                .unwrap(),
            gas_cost_tx: vec![
                (TxFeeTypes::Transfer, BigUint::from(350u32)),
                (TxFeeTypes::Withdraw, BigUint::from(10000u32)),
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
    impl FeeTickerAPI for MockApiProvider {
        fn get_last_quote(
            &self,
            token: TokenLike,
        ) -> Box<dyn Future<Output = Result<TokenPrice, failure::Error>> + Unpin> {
            for test_token in TestToken::all_tokens() {
                if test_token.id == token {
                    let token_price = TokenPrice {
                        usd_price: test_token.price_usd,
                        last_updated: Utc::now(),
                    };
                    return Box::new(futures::future::ok(token_price));
                }
            }
            unreachable!()
        }

        /// Get current gas price in ETH
        fn get_gas_price_gwei(
            &self,
        ) -> Box<dyn Future<Output = Result<BigUint, failure::Error>> + Unpin> {
            Box::new(futures::future::ok(BigUint::from(10u32))) // 10 Gwei
        }
    }

    #[test]
    fn test_ticker_formula() {
        let config = get_test_ticker_config();

        let ticker = FeeTicker::new(MockApiProvider, mpsc::channel(1).1, config.clone());

        let get_token_fee_in_usd = |tx_type: TxFeeTypes, token: TokenLike| -> Ratio<BigUint> {
            let fee_in_token =
                block_on(ticker.get_fee_from_ticker_in_wei_rounded(tx_type, token.clone()))
                    .expect("failed to get fee in token");
            let fee_in_usd = block_on(MockApiProvider.get_last_quote(token.clone()))
                .expect("failed to get fee in usd")
                .usd_price
                / BigUint::from(10u32).pow(18u32)
                * fee_in_token;
            fee_in_usd
        };

        let expected_price_of_eth_token_tx = |fee_type: TxFeeTypes| -> Ratio<BigUint> {
            let zkp_chunk_cost = config.zkp_cost_chunk_usd.clone();
            let gas_price_wei =
                block_on(ticker.api.get_gas_price_gwei()).unwrap() * BigUint::from(10u32).pow(6u32);
            let risk_factor_eth = config
                .tokens_risk_factors
                .get(&TestToken::eth().id)
                .cloned()
                .unwrap_or_else(|| Ratio::<BigUint>::from_integer(1u32.into()));
            let gas_cost_op = config.gas_cost_tx.get(&fee_type).cloned().unwrap();
            let wei_cost_usd = block_on(ticker.api.get_last_quote(TestToken::eth().id))
                .unwrap()
                .usd_price
                / BigUint::from(10u32).pow(18u32);

            let chunks = match fee_type {
                TxFeeTypes::Transfer => TransferOp::CHUNKS,
                TxFeeTypes::Withdraw => WithdrawOp::CHUNKS,
            };

            (zkp_chunk_cost * BigUint::from(chunks as u64)
                + wei_cost_usd * gas_price_wei * gas_cost_op)
                * risk_factor_eth
        };

        let threshold = Ratio::from_integer(BigUint::from(10u32).pow(5u32)).inv();

        {
            let expected_price_of_eth_token_transfer_usd =
                expected_price_of_eth_token_tx(TxFeeTypes::Transfer);
            let expected_price_of_eth_token_withdraw_usd =
                expected_price_of_eth_token_tx(TxFeeTypes::Withdraw);

            for token in TestToken::all_tokens() {
                let transfer_fee = get_token_fee_in_usd(TxFeeTypes::Transfer, token.id.clone());
                let expected_fee =
                    expected_price_of_eth_token_transfer_usd.clone() * token.risk_factor();
                let transfer_diff = std::cmp::max(&transfer_fee, &expected_fee)
                    - std::cmp::min(&transfer_fee, &expected_fee);
                assert!(
                    transfer_diff <= threshold.clone(),
                    "token transfer fee is above eth fee threshold: <{:?}: {}, ETH: {}, diff: {}, threshold: {}>", token.id, 
                    UnsignedRationalUtils::serialize_to_str_with_dot(&transfer_fee,6),
                    UnsignedRationalUtils::serialize_to_str_with_dot(&expected_fee,6),
                    transfer_diff, &threshold);

                let withdraw_fee = get_token_fee_in_usd(TxFeeTypes::Withdraw, token.id.clone());
                let expected_fee =
                    expected_price_of_eth_token_withdraw_usd.clone() * token.risk_factor();
                let withdraw_diff = std::cmp::max(&withdraw_fee, &expected_fee)
                    - std::cmp::min(&withdraw_fee, &expected_fee);
                assert!(
                    withdraw_diff <= threshold.clone(),
                    "token withdraw fee is above eth fee threshold: <{:?}: {}, ETH: {}, diff: {}, threshold: {}>", token.id,
                    UnsignedRationalUtils::serialize_to_str_with_dot(&withdraw_fee,6),
                    UnsignedRationalUtils::serialize_to_str_with_dot(&expected_fee,6),
                    withdraw_diff, &threshold);
            }
        }
    }
}
