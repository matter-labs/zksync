//! Module used to calculate fee for transactions.
//!
//! base formula for calculation:
//! `( zkp cost of chunk * number of chunks + gas price of transaction) * token risk factor / cost of token is usd`

// Built-in deps
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::iter::FromIterator;
// External deps
use bigdecimal::BigDecimal;
use futures::{
    channel::{mpsc::Receiver, oneshot},
    StreamExt,
};
use num::{
    rational::Ratio,
    traits::{Inv, Pow},
    BigUint, Zero,
};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;
use tokio::time::Instant;

// Workspace deps
use zksync_config::{configs::ticker::TokenPriceSource, ZkSyncConfig};
use zksync_storage::ConnectionPool;
use zksync_types::{
    Address, BatchFee, ChangePubKeyOp, Fee, OutputFeeType, Token, TokenId, TokenLike, TransferOp,
    TransferToNewOp, TxFeeTypes, WithdrawOp,
};
use zksync_utils::ratio_to_big_decimal;

// Local deps
use crate::fee_ticker::balancer::TickerBalancer;
use crate::fee_ticker::ticker_info::{FeeTickerInfo, TickerInfo};
use crate::fee_ticker::validator::MarketUpdater;
use crate::fee_ticker::{
    ticker_api::{
        coingecko::CoinGeckoAPI, coinmarkercap::CoinMarketCapAPI, FeeTickerAPI, TickerApi,
        CONNECTION_TIMEOUT,
    },
    validator::{
        watcher::{TokenWatcher, UniswapTokenWatcher},
        FeeTokenValidator,
    },
};
use crate::utils::token_db_cache::TokenDBCache;
use zksync_types::tokens::{ChangePubKeyFeeType, ChangePubKeyFeeTypeArg};

mod constants;
mod ticker_api;
mod ticker_info;
pub mod validator;

mod balancer;
#[cfg(test)]
mod tests;

/// Contains cost of zkSync operations in Wei.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GasOperationsCost {
    standard_cost: HashMap<OutputFeeType, BigUint>,
    subsidize_cost: HashMap<OutputFeeType, BigUint>,
}

impl GasOperationsCost {
    pub fn from_constants(fast_processing_coeff: f64) -> Self {
        // We increase gas price for fast withdrawals, since it will induce generating a smaller block
        // size, resulting in us paying more gas than for bigger block.
        let standard_fast_withdrawal_cost =
            (constants::BASE_WITHDRAW_COST as f64 * fast_processing_coeff) as u32;
        let subsidy_fast_withdrawal_cost =
            (constants::SUBSIDY_WITHDRAW_COST as f64 * fast_processing_coeff) as u32;

        let standard_cost = vec![
            (
                OutputFeeType::Transfer,
                constants::BASE_TRANSFER_COST.into(),
            ),
            (
                OutputFeeType::TransferToNew,
                constants::BASE_TRANSFER_TO_NEW_COST.into(),
            ),
            (
                OutputFeeType::Withdraw,
                constants::BASE_WITHDRAW_COST.into(),
            ),
            (
                OutputFeeType::FastWithdraw,
                standard_fast_withdrawal_cost.into(),
            ),
            (
                OutputFeeType::ChangePubKey(ChangePubKeyFeeTypeArg::PreContracts4Version {
                    onchain_pubkey_auth: false,
                }),
                constants::BASE_OLD_CHANGE_PUBKEY_OFFCHAIN_COST.into(),
            ),
            (
                OutputFeeType::ChangePubKey(ChangePubKeyFeeTypeArg::PreContracts4Version {
                    onchain_pubkey_auth: true,
                }),
                constants::BASE_CHANGE_PUBKEY_ONCHAIN_COST.into(),
            ),
            (
                OutputFeeType::ChangePubKey(ChangePubKeyFeeTypeArg::ContractsV4Version(
                    ChangePubKeyFeeType::Onchain,
                )),
                constants::BASE_CHANGE_PUBKEY_ONCHAIN_COST.into(),
            ),
            (
                OutputFeeType::ChangePubKey(ChangePubKeyFeeTypeArg::ContractsV4Version(
                    ChangePubKeyFeeType::ECDSA,
                )),
                constants::BASE_CHANGE_PUBKEY_OFFCHAIN_COST.into(),
            ),
            (
                OutputFeeType::ChangePubKey(ChangePubKeyFeeTypeArg::ContractsV4Version(
                    ChangePubKeyFeeType::CREATE2,
                )),
                constants::BASE_CHANGE_PUBKEY_OFFCHAIN_COST.into(),
            ),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();

        let subsidize_cost = vec![
            (
                OutputFeeType::Transfer,
                constants::SUBSIDY_TRANSFER_COST.into(),
            ),
            (
                OutputFeeType::TransferToNew,
                constants::SUBSIDY_TRANSFER_TO_NEW_COST.into(),
            ),
            (
                OutputFeeType::Withdraw,
                constants::SUBSIDY_WITHDRAW_COST.into(),
            ),
            (
                OutputFeeType::FastWithdraw,
                subsidy_fast_withdrawal_cost.into(),
            ),
            (
                OutputFeeType::ChangePubKey(ChangePubKeyFeeTypeArg::PreContracts4Version {
                    onchain_pubkey_auth: false,
                }),
                constants::SUBSIDY_CHANGE_PUBKEY_OFFCHAIN_COST.into(),
            ),
            (
                OutputFeeType::ChangePubKey(ChangePubKeyFeeTypeArg::PreContracts4Version {
                    onchain_pubkey_auth: true,
                }),
                constants::SUBSIDY_CHANGE_PUBKEY_OFFCHAIN_COST.into(),
            ),
            (
                OutputFeeType::ChangePubKey(ChangePubKeyFeeTypeArg::ContractsV4Version(
                    ChangePubKeyFeeType::Onchain,
                )),
                constants::BASE_CHANGE_PUBKEY_ONCHAIN_COST.into(),
            ),
            (
                OutputFeeType::ChangePubKey(ChangePubKeyFeeTypeArg::ContractsV4Version(
                    ChangePubKeyFeeType::ECDSA,
                )),
                constants::SUBSIDY_CHANGE_PUBKEY_OFFCHAIN_COST.into(),
            ),
            (
                OutputFeeType::ChangePubKey(ChangePubKeyFeeTypeArg::ContractsV4Version(
                    ChangePubKeyFeeType::CREATE2,
                )),
                constants::SUBSIDY_CHANGE_PUBKEY_OFFCHAIN_COST.into(),
            ),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();

        Self {
            standard_cost,
            subsidize_cost,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TickerConfig {
    zkp_cost_chunk_usd: Ratio<BigUint>,
    gas_cost_tx: GasOperationsCost,
    tokens_risk_factors: HashMap<TokenId, Ratio<BigUint>>,
    not_subsidized_tokens: HashSet<Address>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TokenPriceRequestType {
    USDForOneWei,
    USDForOneToken,
}

#[derive(Debug)]
pub enum TickerRequest {
    GetTxFee {
        tx_type: TxFeeTypes,
        address: Address,
        token: TokenLike,
        response: oneshot::Sender<Result<Fee, anyhow::Error>>,
    },
    GetBatchTxFee {
        transactions: Vec<(TxFeeTypes, Address)>,
        token: TokenLike,
        response: oneshot::Sender<Result<BatchFee, anyhow::Error>>,
    },
    GetTokenPrice {
        token: TokenLike,
        response: oneshot::Sender<Result<BigDecimal, anyhow::Error>>,
        req_type: TokenPriceRequestType,
    },
    IsTokenAllowed {
        token: TokenLike,
        response: oneshot::Sender<Result<bool, anyhow::Error>>,
    },
}

struct FeeTicker<API, INFO, WATCHER> {
    api: API,
    info: INFO,
    requests: Receiver<TickerRequest>,
    config: TickerConfig,
    validator: FeeTokenValidator<WATCHER>,
}

#[must_use]
pub fn run_ticker_task(
    db_pool: ConnectionPool,
    tricker_requests: Receiver<TickerRequest>,
    config: &ZkSyncConfig,
) -> JoinHandle<()> {
    let ticker_config = TickerConfig {
        zkp_cost_chunk_usd: Ratio::from_integer(BigUint::from(10u32).pow(3u32)).inv(),
        gas_cost_tx: GasOperationsCost::from_constants(config.ticker.fast_processing_coeff),
        tokens_risk_factors: HashMap::new(),
        not_subsidized_tokens: HashSet::from_iter(config.ticker.not_subsidized_tokens.clone()),
    };

    let cache = (db_pool.clone(), TokenDBCache::new());
    let watcher = UniswapTokenWatcher::new(config.ticker.uniswap_url.clone());
    let validator = FeeTokenValidator::new(
        cache.clone(),
        chrono::Duration::seconds(config.ticker.available_liquidity_seconds as i64),
        BigDecimal::try_from(config.ticker.liquidity_volume).expect("Valid f64 for decimal"),
        HashSet::from_iter(config.ticker.unconditionally_valid_tokens.clone()),
        watcher.clone(),
    );

    let updater = MarketUpdater::new(cache, watcher);
    tokio::spawn(updater.keep_updated(config.ticker.token_market_update_time));
    let client = reqwest::ClientBuilder::new()
        .timeout(CONNECTION_TIMEOUT)
        .connect_timeout(CONNECTION_TIMEOUT)
        .build()
        .expect("Failed to build reqwest::Client");
    let (price_source, base_url) = config.ticker.price_source();
    match price_source {
        TokenPriceSource::CoinMarketCap => {
            let token_price_api =
                CoinMarketCapAPI::new(client, base_url.parse().expect("Correct CoinMarketCap url"));

            let ticker_api = TickerApi::new(db_pool.clone(), token_price_api);
            let ticker_info = TickerInfo::new(db_pool);
            let fee_ticker = FeeTicker::new(
                ticker_api,
                ticker_info,
                tricker_requests,
                ticker_config,
                validator,
            );

            tokio::spawn(fee_ticker.run())
        }

        TokenPriceSource::CoinGecko => {
            let token_price_api =
                CoinGeckoAPI::new(client, base_url.parse().expect("Correct CoinGecko url"))
                    .expect("failed to init CoinGecko client");
            let ticker_info = TickerInfo::new(db_pool.clone());

            let mut ticker_balancer = TickerBalancer::new(
                token_price_api,
                ticker_info,
                ticker_config,
                validator,
                tricker_requests,
                db_pool,
                config.ticker.number_of_ticker_actors,
            );
            ticker_balancer.spawn_tickers();
            tokio::spawn(ticker_balancer.run())
        }
    }
}

impl<API: FeeTickerAPI, INFO: FeeTickerInfo, WATCHER: TokenWatcher> FeeTicker<API, INFO, WATCHER> {
    fn new(
        api: API,
        info: INFO,
        requests: Receiver<TickerRequest>,
        config: TickerConfig,
        validator: FeeTokenValidator<WATCHER>,
    ) -> Self {
        Self {
            api,
            info,
            requests,
            config,
            validator,
        }
    }

    /// Increases the gas price by a constant coefficient.
    /// Due to the high volatility of gas prices, we are include the risk
    /// in the fee in order not to go into negative territory.
    fn risk_gas_price_estimate(gas_price: BigUint) -> BigUint {
        gas_price * BigUint::from(130u32) / BigUint::from(100u32)
    }

    async fn run(mut self) {
        while let Some(request) = self.requests.next().await {
            let start = Instant::now();
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
                    metrics::histogram!("ticker.get_tx_fee", start.elapsed());
                    response.send(fee).unwrap_or_default()
                }
                TickerRequest::GetTokenPrice {
                    token,
                    response,
                    req_type,
                } => {
                    let price = self.get_token_price(token, req_type).await;
                    metrics::histogram!("ticker.get_token_price", start.elapsed());
                    response.send(price).unwrap_or_default();
                }
                TickerRequest::IsTokenAllowed { token, response } => {
                    let allowed = self.validator.token_allowed(token).await;
                    metrics::histogram!("ticker.is_token_allowed", start.elapsed());
                    response.send(allowed).unwrap_or_default();
                }
                TickerRequest::GetBatchTxFee {
                    transactions,
                    token,
                    response,
                } => {
                    let fee = self.get_batch_from_ticker_in_wei(token, transactions).await;
                    metrics::histogram!("ticker.get_tx_fee", start.elapsed());
                    response.send(fee).unwrap_or_default()
                }
            }
        }
    }

    async fn get_token_price(
        &self,
        token: TokenLike,
        request_type: TokenPriceRequestType,
    ) -> Result<BigDecimal, anyhow::Error> {
        let factor = match request_type {
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

    /// Returns `true` if the token is subsidized.
    fn is_token_subsidized(&self, token: &Token) -> bool {
        // We have disabled the subsidies up until the contract upgrade (when the prices will indeed become that
        // low), but however we want to leave ourselves the possibility to easily enable them if required.
        // Thus:
        // TODO: Remove subsidies completely (ZKS-226)
        let subsidies_enabled = std::env::var("TICKER_SUBSIDIES_ENABLED")
            .map(|val| val == "true")
            .unwrap_or(false);
        if !subsidies_enabled {
            return false;
        }

        !self.config.not_subsidized_tokens.contains(&token.address)
    }

    async fn get_fee_from_ticker_in_wei(
        &mut self,
        tx_type: TxFeeTypes,
        token: TokenLike,
        recipient: Address,
    ) -> Result<Fee, anyhow::Error> {
        let zkp_cost_chunk = self.config.zkp_cost_chunk_usd.clone();
        let token = self.api.get_token(token).await?;

        let gas_price_wei = self.api.get_gas_price_wei().await?;
        let scale_gas_price = Self::risk_gas_price_estimate(gas_price_wei.clone());
        let is_token_subsidized = self.is_token_subsidized(&token);
        let wei_price_usd = self.wei_price_usd().await?;
        let token_usd_risk = self.token_usd_risk(&token).await?;

        let (fee_type, gas_tx_amount, op_chunks) = self
            .gas_tx_amount(is_token_subsidized, tx_type, recipient)
            .await;

        let zkp_fee = (zkp_cost_chunk * op_chunks) * token_usd_risk.clone();
        let gas_fee =
            (wei_price_usd * gas_tx_amount.clone() * scale_gas_price.clone()) * token_usd_risk;

        Ok(Fee::new(
            fee_type,
            zkp_fee,
            gas_fee,
            gas_tx_amount,
            gas_price_wei,
        ))
    }

    async fn get_batch_from_ticker_in_wei(
        &mut self,
        token: TokenLike,
        txs: Vec<(TxFeeTypes, Address)>,
    ) -> anyhow::Result<BatchFee> {
        let zkp_cost_chunk = self.config.zkp_cost_chunk_usd.clone();
        let token = self.api.get_token(token).await?;

        let gas_price_wei = self.api.get_gas_price_wei().await?;
        let scale_gas_price = Self::risk_gas_price_estimate(gas_price_wei.clone());
        let is_token_subsidized = self.is_token_subsidized(&token);
        let wei_price_usd = self.wei_price_usd().await?;
        let token_usd_risk = self.token_usd_risk(&token).await?;

        let mut total_gas_tx_amount = BigUint::zero();
        let mut total_op_chunks = BigUint::zero();

        for (tx_type, recipient) in txs {
            let (_, gas_tx_amount, op_chunks) = self
                .gas_tx_amount(is_token_subsidized, tx_type, recipient)
                .await;
            total_gas_tx_amount += gas_tx_amount;
            total_op_chunks += op_chunks;
        }

        let total_zkp_fee = (zkp_cost_chunk * total_op_chunks) * token_usd_risk.clone();
        let total_gas_fee =
            (wei_price_usd * total_gas_tx_amount * scale_gas_price) * token_usd_risk;
        let total_fee = BatchFee::new(&total_zkp_fee, &total_gas_fee);

        Ok(total_fee)
    }

    async fn wei_price_usd(&mut self) -> anyhow::Result<Ratio<BigUint>> {
        Ok(self
            .api
            .get_last_quote(TokenLike::Id(TokenId(0)))
            .await?
            .usd_price
            / BigUint::from(10u32).pow(18u32))
    }

    async fn token_usd_risk(&mut self, token: &Token) -> anyhow::Result<Ratio<BigUint>> {
        let token_risk_factor = self
            .config
            .tokens_risk_factors
            .get(&token.id)
            .cloned()
            .unwrap_or_else(|| Ratio::from_integer(1u32.into()));

        let token_price_usd = self
            .api
            .get_last_quote(TokenLike::Id(token.id))
            .await?
            .usd_price
            / BigUint::from(10u32).pow(u32::from(token.decimals));
        Ok(token_risk_factor / token_price_usd)
    }

    /// Returns `true` if account does not yet exist in the zkSync network.
    async fn is_account_new(&mut self, address: Address) -> bool {
        self.info.is_account_new(address).await
    }

    async fn gas_tx_amount(
        &mut self,
        is_token_subsidized: bool,
        tx_type: TxFeeTypes,
        recipient: Address,
    ) -> (OutputFeeType, BigUint, BigUint) {
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
            TxFeeTypes::ChangePubKey(arg) => {
                (OutputFeeType::ChangePubKey(arg), ChangePubKeyOp::CHUNKS)
            }
        };
        // Convert chunks amount to `BigUint`.
        let op_chunks = BigUint::from(op_chunks);

        let gas_tx_amount = {
            if is_token_subsidized {
                self.config
                    .gas_cost_tx
                    .subsidize_cost
                    .get(&fee_type)
                    .cloned()
                    .unwrap()
            } else {
                self.config
                    .gas_cost_tx
                    .standard_cost
                    .get(&fee_type)
                    .cloned()
                    .unwrap()
            }
        };
        (fee_type, gas_tx_amount, op_chunks)
    }
}
