//! Module used to calculate fee for transactions.
//!
//! base formula for calculation:
//! `( zkp cost of chunk * number of chunks + gas price of transaction) * token risk factor / cost of token is usd`

// Built-in deps
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::fmt::Display;
use std::iter::FromIterator;
use std::time::Duration;

// External deps
use bigdecimal::BigDecimal;
use num::{
    rational::Ratio,
    traits::{Inv, Pow},
    BigUint, CheckedDiv, Zero,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::task::JoinHandle;
use tokio::time::Instant;

// Workspace deps

use zksync_config::configs::ticker::TokenPriceSource;
use zksync_storage::ConnectionPool;
use zksync_token_db_cache::TokenDBCache;
use zksync_types::{
    gas_counter::GasCounter, tokens::ChangePubKeyFeeTypeArg, tx::ChangePubKeyType, Address,
    BatchFee, ChangePubKeyOp, Fee, MintNFTOp, OutputFeeType, SwapOp, Token, TokenId, TokenLike,
    TransferOp, TransferToNewOp, TxFeeTypes, WithdrawNFTOp, WithdrawOp,
};
use zksync_utils::{big_decimal_to_ratio, ratio_to_big_decimal};

// Local deps
use crate::fee_ticker::constants::AMORTIZED_COST_PER_CHUNK;
pub use crate::fee_ticker::ticker_info::{FeeTickerInfo, TickerInfo};
use crate::fee_ticker::validator::FeeTokenValidator;
use crate::fee_ticker::{
    ticker_api::{
        coingecko::CoinGeckoAPI, coinmarkercap::CoinMarketCapAPI, FeeTickerAPI, TickerApi,
        CONNECTION_TIMEOUT,
    },
    validator::{watcher::UniswapTokenWatcher, MarketUpdater},
};

mod constants;
mod ticker_api;
pub(crate) mod ticker_info;
pub mod validator;

#[cfg(test)]
pub(crate) mod tests;

/// Contains cost of zkSync operations in Wei.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GasOperationsCost {
    standard_cost: HashMap<OutputFeeType, BigUint>,
}

impl GasOperationsCost {
    pub fn from_constants(fast_processing_coeff: f64) -> Self {
        // We increase gas price for fast withdrawals, since it will induce generating a smaller block
        // size, resulting in us paying more gas than for bigger block.
        let standard_fast_withdrawal_cost =
            (constants::BASE_WITHDRAW_COST as f64 * fast_processing_coeff) as u32;
        let standard_fast_withdrawal_nft_cost =
            (constants::BASE_WITHDRAW_NFT_COST as f64 * fast_processing_coeff) as u32;

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
                    ChangePubKeyType::EIP712,
                )),
                // The cost of ECDSA and EIP712 ChangePubKey is almost the same.
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

        Self { standard_cost }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TickerConfig {
    pub zkp_cost_chunk_usd: Ratio<BigUint>,
    pub gas_cost_tx: GasOperationsCost,
    pub tokens_risk_factors: HashMap<TokenId, Ratio<BigUint>>,
    pub scale_fee_coefficient: Ratio<BigUint>,
    pub max_blocks_to_aggregate: u32,
    pub subsidy_cpk_price_usd: Ratio<BigUint>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TokenPriceRequestType {
    USDForOneWei,
    USDForOneToken,
}

#[derive(Debug, Clone)]
pub struct ResponseFee {
    pub normal_fee: Fee,
    pub subsidized_fee: Fee,
    pub subsidy_size_usd: Ratio<BigUint>,
}

#[derive(Debug, Clone)]
pub struct ResponseBatchFee {
    pub normal_fee: BatchFee,
    pub subsidized_fee: BatchFee,
    pub subsidy_size_usd: Ratio<BigUint>,
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

#[derive(Clone)]
pub struct FeeTicker {
    info: Box<dyn FeeTickerInfo>,
    config: TickerConfig,
    validator: FeeTokenValidator,
}

const CPK_CREATE2_FEE_TYPE: OutputFeeType = OutputFeeType::ChangePubKey(
    ChangePubKeyFeeTypeArg::ContractsV4Version(ChangePubKeyType::CREATE2),
);
// Make no more than (Number of tokens) queries per 5 minutes to database is a good result
// for updating names for tokens.
const TOKEN_INVALIDATE_CACHE: Duration = Duration::from_secs(5 * 60);

#[must_use]
pub fn run_updaters(
    db_pool: ConnectionPool,
    config: &zksync_config::TickerConfig,
) -> Vec<JoinHandle<()>> {
    let cache = (db_pool.clone(), TokenDBCache::new(TOKEN_INVALIDATE_CACHE));

    let watcher = UniswapTokenWatcher::new(config.uniswap_url.clone());

    let updater = MarketUpdater::new(cache, watcher);
    let mut tasks = vec![tokio::spawn(
        updater.keep_updated(config.token_market_update_time),
    )];
    let client = reqwest::ClientBuilder::new()
        .timeout(CONNECTION_TIMEOUT)
        .connect_timeout(CONNECTION_TIMEOUT)
        .build()
        .expect("Failed to build reqwest::Client");
    let (price_source, base_url) = config.price_source();
    let price_updater = match price_source {
        TokenPriceSource::CoinMarketCap => {
            let token_price_api =
                CoinMarketCapAPI::new(client, base_url.parse().expect("Correct CoinMarketCap url"));

            let ticker_api = TickerApi::new(db_pool, token_price_api);
            tokio::spawn(ticker_api.keep_price_updated())
        }

        TokenPriceSource::CoinGecko => tokio::spawn(async move {
            let token_price_api =
                CoinGeckoAPI::new(client, base_url.parse().expect("Correct CoinGecko url"))
                    .await
                    .expect("failed to init CoinGecko client");
            let ticker_api = TickerApi::new(db_pool, token_price_api);

            ticker_api.keep_price_updated().await;
        }),
    };
    tasks.push(price_updater);
    tasks
}

impl FeeTicker {
    pub fn new(
        info: Box<dyn FeeTickerInfo>,
        config: TickerConfig,
        validator: FeeTokenValidator,
    ) -> Self {
        Self {
            info,
            config,
            validator,
        }
    }

    pub fn new_with_default_validator(
        info: Box<dyn FeeTickerInfo>,
        config: zksync_config::TickerConfig,
        max_blocks_to_aggregate: u32,
        connection_pool: ConnectionPool,
    ) -> Self {
        let cache = (connection_pool, TokenDBCache::new(TOKEN_INVALIDATE_CACHE));
        let ticker_config = TickerConfig {
            zkp_cost_chunk_usd: Ratio::from_integer(BigUint::from(10u32).pow(3u32)).inv(),
            gas_cost_tx: GasOperationsCost::from_constants(config.fast_processing_coeff),
            tokens_risk_factors: HashMap::new(),
            scale_fee_coefficient: Ratio::new(
                BigUint::from(config.scale_fee_percent),
                BigUint::from(100u32),
            ),
            max_blocks_to_aggregate,
            subsidy_cpk_price_usd: config.subsidy_cpk_price_usd(),
        };
        let validator = FeeTokenValidator::new(
            cache,
            chrono::Duration::seconds(config.available_liquidity_seconds as i64),
            BigDecimal::try_from(config.liquidity_volume).expect("Valid f64 for decimal"),
            HashSet::from_iter(config.unconditionally_valid_tokens),
        );
        Self::new(info, ticker_config, validator)
    }
}

impl FeeTicker {
    /// Increases the gas price by a constant coefficient.
    /// Due to the high volatility of gas prices, we are include the risk
    /// in the fee in order not to go into negative territory.
    fn risk_gas_price_estimate(gas_price: BigUint) -> BigUint {
        gas_price * BigUint::from(130u32) / BigUint::from(100u32)
    }

    pub async fn get_token_price(
        &self,
        token: TokenLike,
        request_type: TokenPriceRequestType,
    ) -> Result<BigDecimal, PriceError> {
        let start = Instant::now();
        let factor = match request_type {
            TokenPriceRequestType::USDForOneWei => {
                let token_decimals = self
                    .info
                    .get_token(token.clone())
                    .await
                    .map_err(PriceError::db_error)?
                    .decimals;
                BigUint::from(10u32).pow(u32::from(token_decimals))
            }
            TokenPriceRequestType::USDForOneToken => BigUint::from(1u32),
        };

        let res = self
            .info
            .get_last_token_price(token)
            .await
            .map(|price| ratio_to_big_decimal(&(price.usd_price / factor), 100));
        metrics::histogram!("ticker.get_token_price", start.elapsed());
        res
    }

    pub async fn get_fee_from_ticker_in_wei(
        &self,
        tx_type: TxFeeTypes,
        token: TokenLike,
        recipient: Address,
    ) -> Result<ResponseFee, anyhow::Error> {
        let start = Instant::now();
        let zkp_cost_chunk = self.config.zkp_cost_chunk_usd.clone();
        let token = self.info.get_token(token).await?;

        let gas_price_wei = self.info.get_gas_price_wei().await?;
        let scale_gas_price = Self::risk_gas_price_estimate(gas_price_wei.clone());
        let wei_price_usd = self.wei_price_usd().await?;
        let token_usd_risk = self.token_usd_risk(&token).await?;

        let (fee_type, gas_tx_amount, op_chunks) = self.gas_tx_amount(tx_type, recipient).await?;

        let zkp_fee = (zkp_cost_chunk * op_chunks) * &token_usd_risk;
        let mut normal_gas_fee =
            (&wei_price_usd * gas_tx_amount.clone() * scale_gas_price.clone()) * &token_usd_risk;

        // Increase fee only for L2 operations
        if matches!(
            fee_type,
            OutputFeeType::TransferToNew
                | OutputFeeType::Transfer
                | OutputFeeType::MintNFT
                | OutputFeeType::Swap
        ) {
            normal_gas_fee *= self.config.scale_fee_coefficient.clone();
        }

        let normal_fee = Fee::new(
            fee_type,
            zkp_fee,
            normal_gas_fee,
            gas_tx_amount,
            gas_price_wei.clone(),
        );

        if fee_type == CPK_CREATE2_FEE_TYPE {
            let token_price = self
                .get_token_price(TokenLike::Id(token.id), TokenPriceRequestType::USDForOneWei)
                .await?;

            // It is safe to do unwrap in the next two lines, because token being acceptable for fees
            // assumes that the token's price is > 0
            let token_price = big_decimal_to_ratio(&token_price).unwrap();
            let full_amount = self
                .config
                .subsidy_cpk_price_usd
                .checked_div(&token_price)
                .unwrap();

            let subsidized_fee = Fee::new(
                fee_type,
                Ratio::from(BigUint::zero()),
                full_amount,
                BigUint::zero(),
                BigUint::zero(),
            );

            let subsidy_size_usd = if normal_fee.total_fee > subsidized_fee.total_fee {
                token_price * (&normal_fee.total_fee - &subsidized_fee.total_fee)
            } else {
                Ratio::from(BigUint::from(0u32))
            };

            return Ok(ResponseFee {
                normal_fee,
                subsidized_fee,
                subsidy_size_usd,
            });
        }

        metrics::histogram!("ticker.get_fee_from_ticker_in_wei", start.elapsed());
        Ok(ResponseFee {
            normal_fee: normal_fee.clone(),
            subsidized_fee: normal_fee,
            subsidy_size_usd: Ratio::from(BigUint::from(0u32)),
        })
    }

    pub async fn get_batch_from_ticker_in_wei(
        &self,
        token: TokenLike,
        txs: Vec<(TxFeeTypes, Address)>,
    ) -> anyhow::Result<ResponseBatchFee> {
        let start = Instant::now();
        let zkp_cost_chunk = self.config.zkp_cost_chunk_usd.clone();

        let token = self.info.get_token(token).await?;

        let gas_price_wei = self.info.get_gas_price_wei().await?;
        let scale_gas_price = Self::risk_gas_price_estimate(gas_price_wei.clone());
        let wei_price_usd = self.wei_price_usd().await?;
        let token_usd_risk = self.token_usd_risk(&token).await?;

        let mut total_normal_gas_tx_amount = Ratio::from(BigUint::zero());
        let mut total_op_chunks = Ratio::from(BigUint::zero());
        let mut total_subsidized_gas_tx_amount = Ratio::from(BigUint::zero());
        let mut total_subsidized_op_chunks = Ratio::from(BigUint::zero());

        /*
            The input of each operation in the batch gas price is the following:
            (&wei_price_usd * gas_amount * &scale_gas_price) * token_usd_risk
            In case we wish to subsidize the tx
            (&wei_price_usd * gas_amount * &scale_gas_price) * token_usd_risk = subsidized_fee_in_token

            Since subsidized fee is denoted in usd
            (&wei_price_usd * gas_amount * &scale_gas_price) * token_usd_risk * token_price = subsidized_fee_in_token * token_price = subsidized_fee_in_usd

            Thus,
            gas_amount = subsidized_fee_in_usd / (&wei_price_usd * &scale_gas_price * token_usd_risk * token_price)
        */
        let token_price = self
            .get_token_price(TokenLike::Id(token.id), TokenPriceRequestType::USDForOneWei)
            .await?;
        let token_price = big_decimal_to_ratio(&token_price).unwrap();

        let denom_part = &wei_price_usd * &scale_gas_price * &token_usd_risk * &token_price;

        let subsidized_gas_amount = if token_price.is_zero() {
            // If the price of the token is zero, than it is not possible to calculate the fee in this token
            // Actually we should never get into this clause, since we divive by the token's price in the calculation of token_usd_risk

            return Err(anyhow::Error::msg("The token is not acceptable for fee"));
        } else if denom_part.is_zero() {
            // If the denom_part is zero, it means that either of wei_price_usd, scale_gas_price, token_usd_risk are equal to 0
            // The total_gas_fee is multiplied by all of these multiples in the final calculation of the fee, so it is safe to return 0 here,
            // since the gas cost would be zero anyway.

            // This would mean that the final subsidized fee is zero. However, this is a very rare ocasion
            Ratio::from(BigUint::zero())
        } else {
            &self.config.subsidy_cpk_price_usd / denom_part
        };

        for (tx_type, recipient) in txs {
            let (output_fee_type, gas_tx_amount, op_chunks) =
                self.gas_tx_amount(tx_type, recipient).await?;
            // Increase fee only for L2 operations
            let gas_tx_amount: Ratio<BigUint> = if matches!(
                output_fee_type,
                OutputFeeType::Transfer
                    | OutputFeeType::TransferToNew
                    | OutputFeeType::Swap
                    | OutputFeeType::MintNFT
            ) {
                self.config.scale_fee_coefficient.clone() * gas_tx_amount
            } else {
                gas_tx_amount.into()
            };

            total_normal_gas_tx_amount += &gas_tx_amount;
            total_op_chunks += &op_chunks;

            if output_fee_type == CPK_CREATE2_FEE_TYPE {
                // The subsidy cost contains only gas cost
                total_subsidized_gas_tx_amount += &subsidized_gas_amount;
            } else {
                // No subsidy applied, so the standard fee goes even for subsidized fee
                total_subsidized_gas_tx_amount += gas_tx_amount;
                total_subsidized_op_chunks += op_chunks;
            }
        }

        let normal_fee = {
            let total_zkp_fee = (&zkp_cost_chunk * total_op_chunks) * &token_usd_risk;
            let total_gas_fee =
                (&wei_price_usd * total_normal_gas_tx_amount * &scale_gas_price) * &token_usd_risk;
            BatchFee::new(total_zkp_fee, total_gas_fee)
        };

        let subsidized_fee = {
            let total_zkp_fee = (zkp_cost_chunk * total_subsidized_op_chunks) * &token_usd_risk;
            let total_gas_fee =
                (&wei_price_usd * total_subsidized_gas_tx_amount * &scale_gas_price)
                    * &token_usd_risk;
            BatchFee::new(total_zkp_fee, total_gas_fee)
        };

        let subsidy_size_usd = if normal_fee.total_fee > subsidized_fee.total_fee {
            token_price * (&normal_fee.total_fee - &subsidized_fee.total_fee)
        } else {
            Ratio::from(BigUint::from(0u32))
        };
        metrics::histogram!("ticker.get_batch_from_ticker_in_wei", start.elapsed());

        Ok(ResponseBatchFee {
            normal_fee,
            subsidized_fee,
            subsidy_size_usd,
        })
    }

    pub async fn wei_price_usd(&self) -> anyhow::Result<Ratio<BigUint>> {
        let start = Instant::now();
        let res = self
            .info
            .get_last_token_price(TokenLike::Id(TokenId(0)))
            .await?
            .usd_price
            / BigUint::from(10u32).pow(18u32);
        metrics::histogram!("ticker.wei_price_usd", start.elapsed());
        Ok(res)
    }

    pub async fn token_usd_risk(&self, token: &Token) -> anyhow::Result<Ratio<BigUint>> {
        let start = Instant::now();
        let token_risk_factor = self
            .config
            .tokens_risk_factors
            .get(&token.id)
            .cloned()
            .unwrap_or_else(|| Ratio::from_integer(1u32.into()));

        let token_price_usd = self
            .info
            .get_last_token_price(TokenLike::Id(token.id))
            .await?
            .usd_price
            / BigUint::from(10u32).pow(u32::from(token.decimals));
        // TODO Check tokens fee allowance by non-zero price (ZKS-580)
        metrics::histogram!("ticker.token_usd_risk", start.elapsed());
        token_risk_factor
            .checked_div(&token_price_usd)
            .ok_or_else(|| anyhow::format_err!("Token is not acceptable for fee"))
    }

    /// Returns `true` if account does not yet exist in the zkSync network.
    pub async fn is_account_new(&self, address: Address) -> anyhow::Result<bool> {
        self.info.is_account_new(address).await
    }

    async fn gas_tx_amount(
        &self,
        tx_type: TxFeeTypes,
        recipient: Address,
    ) -> anyhow::Result<(OutputFeeType, BigUint, BigUint)> {
        let start = Instant::now();
        let (fee_type, op_chunks) = match tx_type {
            TxFeeTypes::Withdraw => (OutputFeeType::Withdraw, WithdrawOp::CHUNKS),
            TxFeeTypes::FastWithdraw => (OutputFeeType::FastWithdraw, WithdrawOp::CHUNKS),
            TxFeeTypes::WithdrawNFT => (OutputFeeType::WithdrawNFT, WithdrawNFTOp::CHUNKS),
            TxFeeTypes::FastWithdrawNFT => (OutputFeeType::FastWithdrawNFT, WithdrawNFTOp::CHUNKS),
            TxFeeTypes::Transfer => {
                if self.is_account_new(recipient).await? {
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

        let gas_tx_amount = if matches!(
            fee_type,
            OutputFeeType::FastWithdraw | OutputFeeType::FastWithdrawNFT
        ) {
            self.calculate_fast_withdrawal_gas_cost(op_chunks).await?
        } else {
            self.config
                .gas_cost_tx
                .standard_cost
                .get(&fee_type)
                .cloned()
                .unwrap()
        };

        // Convert chunks amount to `BigUint`.
        let op_chunks = BigUint::from(op_chunks);
        metrics::histogram!("ticker.gas_tx_amount", start.elapsed());
        Ok((fee_type, gas_tx_amount, op_chunks))
    }
    async fn calculate_fast_withdrawal_gas_cost(
        &self,
        chunk_size: usize,
    ) -> anyhow::Result<BigUint> {
        let start = Instant::now();
        let future_blocks = self.info.blocks_in_future_aggregated_operations().await?;
        let remaining_pending_chunks = self.info.remaining_chunks_in_pending_block().await?;
        let additional_cost = remaining_pending_chunks.map_or(0, |chunks| {
            if chunk_size > chunks {
                0
            } else {
                chunks * AMORTIZED_COST_PER_CHUNK as usize
            }
        });

        // We have to calculate how much from base price for operations has already paid in blocks and add remain cost to fast withdrawal operation
        let commit_cost = calculate_cost(
            GasCounter::BASE_COMMIT_BLOCKS_TX_COST,
            self.config.max_blocks_to_aggregate,
            future_blocks.blocks_to_commit,
        );
        let execute_cost = calculate_cost(
            GasCounter::BASE_EXECUTE_BLOCKS_TX_COST,
            self.config.max_blocks_to_aggregate,
            future_blocks.blocks_to_execute,
        );
        let proof_cost = calculate_cost(
            GasCounter::BASE_PROOF_BLOCKS_TX_COST,
            self.config.max_blocks_to_aggregate,
            future_blocks.blocks_to_prove,
        );
        metrics::histogram!("ticker.calculate_fast_withdrawal_gas_cost", start.elapsed());
        Ok(BigUint::from(
            commit_cost + execute_cost + proof_cost + additional_cost,
        ))
    }

    pub async fn token_allowed_for_fees(&self, token: TokenLike) -> anyhow::Result<bool> {
        self.validator.token_allowed(token).await
    }
}

fn calculate_cost(base_cost: usize, max_blocks: u32, future_blocks: u32) -> usize {
    base_cost - (base_cost / max_blocks as usize) * future_blocks.rem_euclid(max_blocks) as usize
}
