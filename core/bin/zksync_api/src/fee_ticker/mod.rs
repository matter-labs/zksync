//! Module used to calculate fee for transactions.
//!
//! base formula for calculation:
//! `( zkp cost of chunk * number of chunks + gas price of transaction) * token risk factor / cost of token is usd`

// Built-in deps
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::fmt::Display;
use std::iter::FromIterator;
use std::sync::Arc;
// External deps
use bigdecimal::BigDecimal;
use futures::{
    channel::{mpsc::Receiver, oneshot},
    StreamExt,
};
use num::{
    bigint::ToBigInt,
    rational::Ratio,
    traits::{Inv, Pow},
    BigUint, CheckedDiv, CheckedSub, Zero,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::Instant;
// Workspace deps
use zksync_balancer::{Balancer, BuildBalancedItem};
use zksync_config::{configs::ticker::TokenPriceSource, ZkSyncConfig};
use zksync_storage::ConnectionPool;
use zksync_types::{
    tokens::ChangePubKeyFeeTypeArg, tx::ChangePubKeyType, Address, BatchFee, ChangePubKeyOp, Fee,
    MintNFTOp, OutputFeeType, SwapOp, Token, TokenId, TokenLike, TransferOp, TransferToNewOp,
    TxFeeTypes, WithdrawNFTOp, WithdrawOp,
};
use zksync_utils::ratio_to_big_decimal;

// Local deps
use crate::fee_ticker::{
    ticker_api::{
        coingecko::CoinGeckoAPI, coinmarkercap::CoinMarketCapAPI, FeeTickerAPI, TickerApi,
        CONNECTION_TIMEOUT,
    },
    ticker_info::{FeeTickerInfo, TickerInfo},
    validator::{
        watcher::{TokenWatcher, UniswapTokenWatcher},
        FeeTokenValidator, MarketUpdater,
    },
};
use crate::utils::token_db_cache::TokenDBCache;

mod constants;
mod ticker_api;
mod ticker_info;
pub mod validator;

#[cfg(test)]
mod tests;

static TICKER_CHANNEL_SIZE: usize = 32000;

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
        let standard_fast_withdrawal_nft_cost =
            (constants::BASE_WITHDRAW_NFT_COST as f64 * fast_processing_coeff) as u32;
        let subsidy_fast_withdrawal_nft_cost =
            (constants::SUBSIDY_WITHDRAW_NFT_COST as f64 * fast_processing_coeff) as u32;

        let standard_cost = vec![
            (
                OutputFeeType::Transfer,
                constants::BASE_TRANSFER_COST.into(),
            ),
            (OutputFeeType::MintNFT, constants::BASE_MINT_NFT_COST.into()),
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
            (OutputFeeType::Swap, constants::BASE_SWAP_COST.into()),
            (
                OutputFeeType::WithdrawNFT,
                constants::BASE_WITHDRAW_NFT_COST.into(),
            ),
            (
                OutputFeeType::FastWithdrawNFT,
                standard_fast_withdrawal_nft_cost.into(),
            ),
            (OutputFeeType::MintNFT, constants::BASE_MINT_NFT_COST.into()),
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
                    ChangePubKeyType::Onchain,
                )),
                constants::BASE_CHANGE_PUBKEY_ONCHAIN_COST.into(),
            ),
            (
                OutputFeeType::ChangePubKey(ChangePubKeyFeeTypeArg::ContractsV4Version(
                    ChangePubKeyType::ECDSA,
                )),
                constants::BASE_CHANGE_PUBKEY_OFFCHAIN_COST.into(),
            ),
            (
                OutputFeeType::ChangePubKey(ChangePubKeyFeeTypeArg::ContractsV4Version(
                    ChangePubKeyType::CREATE2,
                )),
                constants::BASE_CHANGE_PUBKEY_CREATE2_COST.into(),
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
            (OutputFeeType::Swap, constants::SUBSIDY_SWAP_COST.into()),
            (
                OutputFeeType::WithdrawNFT,
                constants::SUBSIDY_WITHDRAW_NFT_COST.into(),
            ),
            (
                OutputFeeType::FastWithdrawNFT,
                subsidy_fast_withdrawal_nft_cost.into(),
            ),
            (
                OutputFeeType::ChangePubKey(ChangePubKeyFeeTypeArg::PreContracts4Version {
                    onchain_pubkey_auth: false,
                }),
                constants::SUBSIDY_OLD_CHANGE_PUBKEY_OFFCHAIN_COST.into(),
            ),
            (
                OutputFeeType::ChangePubKey(ChangePubKeyFeeTypeArg::PreContracts4Version {
                    onchain_pubkey_auth: true,
                }),
                constants::SUBSIDY_CHANGE_PUBKEY_ONCHAIN_COST.into(),
            ),
            (
                OutputFeeType::ChangePubKey(ChangePubKeyFeeTypeArg::ContractsV4Version(
                    ChangePubKeyType::Onchain,
                )),
                constants::SUBSIDY_CHANGE_PUBKEY_ONCHAIN_COST.into(),
            ),
            (
                OutputFeeType::ChangePubKey(ChangePubKeyFeeTypeArg::ContractsV4Version(
                    ChangePubKeyType::ECDSA,
                )),
                constants::SUBSIDY_CHANGE_PUBKEY_OFFCHAIN_COST.into(),
            ),
            (
                OutputFeeType::ChangePubKey(ChangePubKeyFeeTypeArg::ContractsV4Version(
                    ChangePubKeyType::CREATE2,
                )),
                constants::SUBSIDY_CHANGE_PUBKEY_CREATE2_COST.into(),
            ),
            (
                OutputFeeType::MintNFT,
                constants::SUBSIDY_MINT_NFT_COST.into(),
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
pub struct ResponseFee {
    pub normal_fee: Fee,
    pub subsidy_fee: Fee,
    pub subsidy_size_usd: Ratio<BigUint>,
}

fn get_max_subsidy(
    max_subsidy_usd: &Ratio<BigUint>,
    subsidy_usd: &Ratio<BigUint>,
    normal_fee_token: &BigUint,
    subsidy_fee_token: &BigUint,
) -> BigDecimal {
    if max_subsidy_usd > subsidy_usd {
        let subsidy_uint = normal_fee_token
            .checked_sub(subsidy_fee_token)
            .unwrap_or_else(|| {
                vlog::error!(
                    "Subisdy fee is bigger then normal fee, subsidy: {:?}, noraml_fee: {:?}",
                    subsidy_fee_token,
                    normal_fee_token
                );
                0u32.into()
            });

        BigDecimal::from(
            subsidy_uint
                .to_bigint()
                .expect("biguint should convert to bigint"),
        )
    } else {
        BigDecimal::from(0)
    }
}

impl ResponseFee {
    pub fn get_max_subsidy(&self, allowed_subsidy: &Ratio<BigUint>) -> BigDecimal {
        get_max_subsidy(
            allowed_subsidy,
            &self.subsidy_size_usd,
            &self.normal_fee.total_fee,
            &self.subsidy_fee.total_fee,
        )
    }
}

#[derive(Debug)]
pub struct ResponseBatchFee {
    pub normal_fee: BatchFee,
    pub subsidy_fee: BatchFee,
    pub subsidy_size_usd: Ratio<BigUint>,
}

impl ResponseBatchFee {
    pub fn get_max_subsidy(&self, allowed_subsidy: &Ratio<BigUint>) -> BigDecimal {
        get_max_subsidy(
            allowed_subsidy,
            &self.subsidy_size_usd,
            &self.normal_fee.total_fee,
            &self.subsidy_fee.total_fee,
        )
    }
}

#[derive(Debug)]
pub enum TickerRequest {
    GetTxFee {
        tx_type: TxFeeTypes,
        address: Address,
        token: TokenLike,
        response: oneshot::Sender<Result<ResponseFee, anyhow::Error>>,
    },
    GetBatchTxFee {
        transactions: Vec<(TxFeeTypes, Address)>,
        token: TokenLike,
        response: oneshot::Sender<Result<ResponseBatchFee, anyhow::Error>>,
    },
    GetTokenPrice {
        token: TokenLike,
        response: oneshot::Sender<Result<BigDecimal, PriceError>>,
        req_type: TokenPriceRequestType,
    },
    IsTokenAllowed {
        token: TokenLike,
        response: oneshot::Sender<Result<bool, anyhow::Error>>,
    },
}

#[derive(Debug, Error)]
pub enum PriceError {
    #[error("Token not found: {0}")]
    TokenNotFound(String),
    #[error("Api error: {0}")]
    ApiError(String),
    #[error("Database error: {0}")]
    DBError(String),
}

impl PriceError {
    pub fn token_not_found(msg: impl Display) -> Self {
        Self::TokenNotFound(msg.to_string())
    }

    pub fn api_error(msg: impl Display) -> Self {
        Self::ApiError(msg.to_string())
    }

    pub fn db_error(msg: impl Display) -> Self {
        Self::DBError(msg.to_string())
    }
}

struct FeeTicker<API, INFO, WATCHER> {
    api: API,
    info: INFO,
    requests: Receiver<TickerRequest>,
    config: TickerConfig,
    validator: FeeTokenValidator<WATCHER>,
}

struct FeeTickerBuilder<API, INFO, WATCHER> {
    api: API,
    info: INFO,
    config: TickerConfig,
    validator: FeeTokenValidator<WATCHER>,
}

impl<API: Clone, INFO: Clone, WATCHER: Clone>
    BuildBalancedItem<TickerRequest, FeeTicker<API, INFO, WATCHER>>
    for FeeTickerBuilder<API, INFO, WATCHER>
{
    fn build_with_receiver(
        &self,
        receiver: Receiver<TickerRequest>,
    ) -> FeeTicker<API, INFO, WATCHER> {
        FeeTicker {
            api: self.api.clone(),
            info: self.info.clone(),
            requests: receiver,
            config: self.config.clone(),
            validator: self.validator.clone(),
        }
    }
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

            let token_db_cache = TokenDBCache::new();
            let price_cache = Arc::new(Mutex::new(HashMap::new()));
            let gas_price_cache = Arc::new(Mutex::new(None));
            let ticker_api = TickerApi::new(db_pool, token_price_api)
                .with_token_db_cache(token_db_cache)
                .with_price_cache(price_cache)
                .with_gas_price_cache(gas_price_cache);

            let (ticker_balancer, tickers) = Balancer::new(
                FeeTickerBuilder {
                    api: ticker_api,
                    info: ticker_info,
                    config: ticker_config,
                    validator,
                },
                tricker_requests,
                config.ticker.number_of_ticker_actors,
                TICKER_CHANNEL_SIZE,
            );
            for ticker in tickers.into_iter() {
                tokio::spawn(ticker.run());
            }
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
    ) -> Result<BigDecimal, PriceError> {
        let factor = match request_type {
            TokenPriceRequestType::USDForOneWei => {
                let token_decimals = self
                    .api
                    .get_token(token.clone())
                    .await
                    .map_err(PriceError::db_error)?
                    .decimals;
                BigUint::from(10u32).pow(u32::from(token_decimals))
            }
            TokenPriceRequestType::USDForOneToken => BigUint::from(1u32),
        };

        self.api
            .get_last_quote(token)
            .await
            .map(|price| ratio_to_big_decimal(&(price.usd_price / factor), 100))
    }

    async fn get_fee_from_ticker_in_wei(
        &mut self,
        tx_type: TxFeeTypes,
        token: TokenLike,
        recipient: Address,
    ) -> Result<ResponseFee, anyhow::Error> {
        let zkp_cost_chunk = self.config.zkp_cost_chunk_usd.clone();
        let token = self.api.get_token(token).await?;

        let gas_price_wei = self.api.get_gas_price_wei().await?;
        let scale_gas_price = Self::risk_gas_price_estimate(gas_price_wei.clone());
        let wei_price_usd = self.wei_price_usd().await?;
        let token_usd_risk = self.token_usd_risk(&token).await?;

        let (fee_type, (normal_gas_tx_amount, subsidy_gas_tx_amount), op_chunks) =
            self.gas_tx_amount(tx_type, recipient).await;

        let zkp_fee = (zkp_cost_chunk * op_chunks) * &token_usd_risk;
        let normal_gas_fee =
            (&wei_price_usd * normal_gas_tx_amount.clone() * scale_gas_price.clone())
                * &token_usd_risk;
        let subsidy_gas_fee =
            (wei_price_usd * subsidy_gas_tx_amount.clone() * scale_gas_price.clone())
                * &token_usd_risk;

        let normal_fee = Fee::new(
            fee_type,
            zkp_fee.clone(),
            normal_gas_fee,
            normal_gas_tx_amount,
            gas_price_wei.clone(),
        );

        let subsidy_fee = Fee::new(
            fee_type,
            zkp_fee,
            subsidy_gas_fee,
            subsidy_gas_tx_amount,
            gas_price_wei,
        );

        let subsidy_size_usd =
            Ratio::from_integer(&normal_fee.total_fee - &subsidy_fee.total_fee) / &token_usd_risk;
        Ok(ResponseFee {
            normal_fee,
            subsidy_fee,
            subsidy_size_usd,
        })
    }

    async fn get_batch_from_ticker_in_wei(
        &mut self,
        token: TokenLike,
        txs: Vec<(TxFeeTypes, Address)>,
    ) -> anyhow::Result<ResponseBatchFee> {
        let zkp_cost_chunk = self.config.zkp_cost_chunk_usd.clone();
        let token = self.api.get_token(token).await?;

        let gas_price_wei = self.api.get_gas_price_wei().await?;
        let scale_gas_price = Self::risk_gas_price_estimate(gas_price_wei.clone());
        let wei_price_usd = self.wei_price_usd().await?;
        let token_usd_risk = self.token_usd_risk(&token).await?;

        let mut total_normal_gas_tx_amount = BigUint::zero();
        let mut total_subsidy_gas_tx_amount = BigUint::zero();
        let mut total_op_chunks = BigUint::zero();

        for (tx_type, recipient) in txs {
            let (_, (normal_gas_tx_amount, subsidy_gas_tx_amount), op_chunks) =
                self.gas_tx_amount(tx_type, recipient).await;
            total_normal_gas_tx_amount += normal_gas_tx_amount;
            total_subsidy_gas_tx_amount += subsidy_gas_tx_amount;
            total_op_chunks += op_chunks;
        }

        let total_zkp_fee = (zkp_cost_chunk * total_op_chunks) * token_usd_risk.clone();
        let total_normal_gas_fee =
            (&wei_price_usd * total_normal_gas_tx_amount * &scale_gas_price) * &token_usd_risk;
        let total_subsidy_gas_fee =
            (wei_price_usd * total_subsidy_gas_tx_amount * scale_gas_price) * &token_usd_risk;
        let normal_fee = BatchFee::new(&total_zkp_fee, &total_normal_gas_fee);
        let subsidy_fee = BatchFee::new(&total_zkp_fee, &total_subsidy_gas_fee);

        let subsidy_size_usd =
            Ratio::from_integer(&normal_fee.total_fee - &subsidy_fee.total_fee) / &token_usd_risk;
        Ok(ResponseBatchFee {
            normal_fee,
            subsidy_fee,
            subsidy_size_usd,
        })
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
        // TODO Check tokens fee allowance by non-zero price (ZKS-580)
        token_risk_factor
            .checked_div(&token_price_usd)
            .ok_or_else(|| anyhow::format_err!("Token is not acceptable for fee"))
    }

    /// Returns `true` if account does not yet exist in the zkSync network.
    async fn is_account_new(&mut self, address: Address) -> bool {
        self.info.is_account_new(address).await
    }

    async fn gas_tx_amount(
        &mut self,
        tx_type: TxFeeTypes,
        recipient: Address,
    ) -> (OutputFeeType, (BigUint, BigUint), BigUint) {
        let (fee_type, op_chunks) = match tx_type {
            TxFeeTypes::Withdraw => (OutputFeeType::Withdraw, WithdrawOp::CHUNKS),
            TxFeeTypes::FastWithdraw => (OutputFeeType::FastWithdraw, WithdrawOp::CHUNKS),
            TxFeeTypes::WithdrawNFT => (OutputFeeType::WithdrawNFT, WithdrawNFTOp::CHUNKS),
            TxFeeTypes::FastWithdrawNFT => (OutputFeeType::FastWithdrawNFT, WithdrawNFTOp::CHUNKS),
            TxFeeTypes::Transfer => {
                if self.is_account_new(recipient).await {
                    (OutputFeeType::TransferToNew, TransferToNewOp::CHUNKS)
                } else {
                    (OutputFeeType::Transfer, TransferOp::CHUNKS)
                }
            }
            TxFeeTypes::Swap => (OutputFeeType::Swap, SwapOp::CHUNKS),
            TxFeeTypes::ChangePubKey(arg) => {
                (OutputFeeType::ChangePubKey(arg), ChangePubKeyOp::CHUNKS)
            }
            TxFeeTypes::MintNFT => (OutputFeeType::MintNFT, MintNFTOp::CHUNKS),
        };
        // Convert chunks amount to `BigUint`.
        let op_chunks = BigUint::from(op_chunks);

        let gas_tx_amount = (
            self.config
                .gas_cost_tx
                .standard_cost
                .get(&fee_type)
                .cloned()
                .unwrap(),
            self.config
                .gas_cost_tx
                .subsidize_cost
                .get(&fee_type)
                .cloned()
                .unwrap(),
        );
        (fee_type, gas_tx_amount, op_chunks)
    }
}
