//! Helper module to submit transactions into the zkSync Network.

// Built-in uses
use std::iter::FromIterator;
use std::sync::{Arc, RwLock};
use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    str::FromStr,
};

// External uses
use bigdecimal::BigDecimal;
use chrono::Utc;
use futures::{
    channel::{mpsc, oneshot},
    prelude::*,
};
use itertools::izip;
use num::{bigint::ToBigInt, rational::Ratio, BigUint, CheckedSub, Zero};
use thiserror::Error;

// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_storage::{chain::account::records::EthAccountType, ConnectionPool};
use zksync_types::{
    tx::{
        EthBatchSignData, EthBatchSignatures, EthSignData, Order, SignedZkSyncTx, TxEthSignature,
        TxEthSignatureVariant, TxHash,
    },
    AccountId, Address, BatchFee, Fee, Token, TokenId, TokenLike, TxFeeTypes, ZkSyncTx, H160,
};
use zksync_utils::ratio_to_big_decimal;

// Local uses
use crate::{
    api_server::forced_exit_checker::{ForcedExitAccountAgeChecker, ForcedExitChecker},
    api_server::rpc_server::types::TxWithSignature,
    core_api_client::CoreApiClient,
    fee_ticker::{ResponseBatchFee, ResponseFee, TickerRequest, TokenPriceRequestType},
    signature_checker::{
        BatchRequest, OrderRequest, RequestData, TxRequest, VerifiedTx, VerifySignatureRequest,
    },
    tx_error::TxAddError,
    utils::{block_details_cache::BlockDetailsCache, token_db_cache::TokenDBCache},
};

#[derive(Clone)]
pub struct TxSender {
    pub core_api_client: CoreApiClient,
    pub sign_verify_requests: mpsc::Sender<VerifySignatureRequest>,
    pub ticker_requests: mpsc::Sender<TickerRequest>,

    pub pool: ConnectionPool,
    pub tokens: TokenDBCache,

    pub forced_exit_checker: ForcedExitChecker,
    pub blocks: BlockDetailsCache,
    /// List of account IDs that do not have to pay fees for operations.
    pub fee_free_accounts: HashSet<AccountId>,
    pub enforce_pubkey_change_fee: bool,
    // Limit the number of both transactions and Ethereum signatures per batch.
    pub max_number_of_transactions_per_batch: usize,
    pub max_number_of_authors_per_batch: usize,

    pub subsidy_accumulator: SubsidyAccumulator,
}

/// Used to store paid subsidy and daily limit
#[derive(Debug)]
pub struct SubsidyAccumulator {
    /// Subsidy limit for token address in USD (e.g. 1 -> 1 USD)
    daily_limits: HashMap<Address, Ratio<BigUint>>,
    /// Paid subsidy per token, dated by time when first subsidy is paid
    #[allow(clippy::type_complexity)]
    limit_used: Arc<RwLock<HashMap<Address, (Ratio<BigUint>, chrono::DateTime<Utc>)>>>,
}

impl Clone for SubsidyAccumulator {
    fn clone(&self) -> Self {
        Self {
            daily_limits: self.daily_limits.clone(),
            limit_used: Arc::clone(&self.limit_used),
        }
    }
}

impl SubsidyAccumulator {
    pub fn new(daily_limits: HashMap<Address, Ratio<BigUint>>) -> Self {
        Self {
            daily_limits,
            limit_used: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_allowed_subsidy(&self, token_address: &Address) -> Ratio<BigUint> {
        let limit = self
            .daily_limits
            .get(token_address)
            .cloned()
            .unwrap_or_else(|| Ratio::from_integer(0u32.into()));
        let used = self.limit_used.read().expect("subsidy counter rlock");
        let subsidy_used = used
            .get(token_address)
            .map(|(subs, _)| subs.clone())
            .unwrap_or_else(|| Ratio::from_integer(0u32.into()));
        limit
            .checked_sub(&subsidy_used)
            .unwrap_or_else(|| Ratio::from_integer(0u32.into()))
    }

    pub fn get_total_paid_subsidy(&self, token_address: &Address) -> Ratio<BigUint> {
        let used = self.limit_used.read().expect("subsidy counter rlock");
        used.get(token_address)
            .map(|(subs, _)| subs.clone())
            .unwrap_or_else(|| Ratio::from_integer(0u32.into()))
    }

    pub fn add_used_subsidy(&self, token_address: &Address, subsidy_amount: Ratio<BigUint>) {
        let mut used = self.limit_used.write().expect("subsidy counter wlock");
        let new_value = if let Some((mut old_amount, creation_time)) = used.remove(token_address) {
            if Utc::now().signed_duration_since(creation_time) >= chrono::Duration::days(1) {
                (Ratio::from_integer(0u32.into()), Utc::now())
            } else {
                old_amount += subsidy_amount;
                (old_amount, creation_time)
            }
        } else {
            (Ratio::from_integer(0u32.into()), Utc::now())
        };

        used.insert(*token_address, new_value);
    }
}

#[derive(Debug, Error)]
pub enum SubmitError {
    #[error("Account close tx is disabled.")]
    AccountCloseDisabled,
    #[error("Invalid params: {0}.")]
    InvalidParams(String),
    #[error("Fast processing available only for 'withdraw' operation type.")]
    UnsupportedFastProcessing,
    #[error("Incorrect transaction: {0}.")]
    IncorrectTx(String),
    #[error("Transaction adding error: {0}.")]
    TxAdd(TxAddError),
    #[error("Chosen token is not suitable for paying fees.")]
    InappropriateFeeToken,

    #[error("Communication error with the core server: {0}.")]
    CommunicationCoreServer(String),
    #[error("Internal error.")]
    Internal(anyhow::Error),
    #[error("{0}")]
    Other(String),
}

impl SubmitError {
    pub fn internal(inner: impl Into<anyhow::Error>) -> Self {
        Self::Internal(inner.into())
    }

    pub fn other(msg: impl Display) -> Self {
        Self::Other(msg.to_string())
    }

    pub fn communication_core_server(msg: impl Display) -> Self {
        Self::CommunicationCoreServer(msg.to_string())
    }

    pub fn invalid_params(msg: impl Display) -> Self {
        Self::InvalidParams(msg.to_string())
    }
}

#[macro_export]
macro_rules! internal_error {
    ($err:tt, $input:tt) => {{
        vlog::warn!("Internal Server error: {}, input: {:?}", $err, $input);
        SubmitError::internal($err)
    }};

    ($err:tt) => {{
        internal_error!($err, "N/A")
    }};
}

impl TxSender {
    pub fn new(
        connection_pool: ConnectionPool,
        sign_verify_request_sender: mpsc::Sender<VerifySignatureRequest>,
        ticker_request_sender: mpsc::Sender<TickerRequest>,
        config: &ZkSyncConfig,
    ) -> Self {
        let core_api_client = CoreApiClient::new(config.api.private.url.clone());

        Self::with_client(
            core_api_client,
            connection_pool,
            sign_verify_request_sender,
            ticker_request_sender,
            config,
        )
    }

    pub(crate) fn with_client(
        core_api_client: CoreApiClient,
        connection_pool: ConnectionPool,
        sign_verify_request_sender: mpsc::Sender<VerifySignatureRequest>,
        ticker_request_sender: mpsc::Sender<TickerRequest>,
        config: &ZkSyncConfig,
    ) -> Self {
        let max_number_of_transactions_per_batch =
            config.api.common.max_number_of_transactions_per_batch as usize;
        let max_number_of_authors_per_batch =
            config.api.common.max_number_of_authors_per_batch as usize;

        let subsidy_accumulator = SubsidyAccumulator::new(config.ticker.get_subsidy_limits());

        Self {
            core_api_client,
            pool: connection_pool,
            sign_verify_requests: sign_verify_request_sender,
            ticker_requests: ticker_request_sender,
            tokens: TokenDBCache::new(),
            forced_exit_checker: ForcedExitChecker::new(config),
            enforce_pubkey_change_fee: config.api.common.enforce_pubkey_change_fee,
            blocks: BlockDetailsCache::new(config.api.common.caches_size),

            fee_free_accounts: HashSet::from_iter(config.api.common.fee_free_accounts.clone()),
            max_number_of_transactions_per_batch,
            max_number_of_authors_per_batch,
            subsidy_accumulator,
        }
    }

    /// If `ForcedExit` has Ethereum siganture (e.g. it's a part of a batch), an actual signer
    /// is initiator, not the target, thus, this function will perform a database query to acquire
    /// the corresponding address.
    async fn get_tx_sender(&self, tx: &ZkSyncTx) -> Result<Address, anyhow::Error> {
        match tx {
            ZkSyncTx::ForcedExit(tx) => self.get_address_by_id(tx.initiator_account_id).await,
            _ => Ok(tx.account()),
        }
    }

    async fn get_address_by_id(&self, id: AccountId) -> Result<Address, anyhow::Error> {
        self.pool
            .access_storage()
            .await?
            .chain()
            .account_schema()
            .account_address_by_id(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Order signer account id not found in db"))
    }

    async fn get_tx_sender_type(&self, tx: &ZkSyncTx) -> Result<EthAccountType, SubmitError> {
        self.get_sender_type(tx.account_id().or(Err(SubmitError::AccountCloseDisabled))?)
            .await
    }

    async fn get_sender_type(&self, id: AccountId) -> Result<EthAccountType, SubmitError> {
        Ok(self
            .pool
            .access_storage()
            .await
            .map_err(|_| SubmitError::TxAdd(TxAddError::DbError))?
            .chain()
            .account_schema()
            .account_type_by_id(id)
            .await
            .map_err(|_| SubmitError::TxAdd(TxAddError::DbError))?
            .unwrap_or(EthAccountType::Owned))
    }

    async fn verify_order_eth_signature(
        &self,
        order: &Order,
        signature: Option<TxEthSignature>,
    ) -> Result<(), SubmitError> {
        let signer_type = self.get_sender_type(order.account_id).await?;
        if matches!(signer_type, EthAccountType::CREATE2) {
            return if signature.is_some() {
                Err(SubmitError::IncorrectTx(
                    "Eth signature from CREATE2 account not expected".to_string(),
                ))
            } else {
                Ok(())
            };
        }
        let signature = signature.ok_or(SubmitError::TxAdd(TxAddError::MissingEthSignature))?;
        let signer = self
            .get_address_by_id(order.account_id)
            .await
            .or(Err(SubmitError::TxAdd(TxAddError::DbError)))?;

        let token_sell = self.token_info_from_id(order.token_sell).await?;
        let token_buy = self.token_info_from_id(order.token_buy).await?;
        let message = order
            .get_ethereum_sign_message(&token_sell.symbol, &token_buy.symbol, token_sell.decimals)
            .into_bytes();
        let eth_sign_data = EthSignData { signature, message };
        let (sender, receiever) = oneshot::channel();

        let request = VerifySignatureRequest {
            data: RequestData::Order(OrderRequest {
                order: Box::new(order.clone()),
                sign_data: eth_sign_data,
                sender: signer,
            }),
            response: sender,
        };

        send_verify_request_and_recv(request, self.sign_verify_requests.clone(), receiever).await?;
        Ok(())
    }

    pub async fn submit_tx(
        &self,
        mut tx: ZkSyncTx,
        signature: TxEthSignatureVariant,
        fast_processing: Option<bool>,
    ) -> Result<TxHash, SubmitError> {
        if tx.is_close() {
            return Err(SubmitError::AccountCloseDisabled);
        }

        if let ZkSyncTx::ForcedExit(forced_exit) = &tx {
            self.check_forced_exit(forced_exit).await?;
        }

        let fast_processing = fast_processing.unwrap_or_default(); // `None` => false
        if fast_processing && !tx.is_withdraw() {
            return Err(SubmitError::UnsupportedFastProcessing);
        }

        if let ZkSyncTx::Withdraw(withdraw) = &mut tx {
            if withdraw.fast {
                // We set `fast` field ourselves, so we have to check that user did not set it themselves.
                return Err(SubmitError::IncorrectTx(
                    "'fast' field of Withdraw transaction must not be set manually.".to_string(),
                ));
            }

            // `fast` field is not used in serializing (as it's an internal server option,
            // not the actual transaction part), so we have to set it manually depending on
            // the RPC method input.
            withdraw.fast = fast_processing;
        }

        // Resolve the token.
        let token = self.token_info_from_id(tx.token_id()).await?;
        let allowed_subsidy = self.subsidy_accumulator.get_allowed_subsidy(&token.address);
        let mut paid_subsidy = Ratio::from_integer(0u32.into());
        let msg_to_sign = tx
            .get_ethereum_sign_message(token.clone())
            .map(String::into_bytes);

        let is_whitelisted_initiator = tx
            .account_id()
            .map(|account_id| self.fee_free_accounts.contains(&account_id))
            .unwrap_or(false);

        let tx_fee_info = if !is_whitelisted_initiator {
            tx.get_fee_info()
        } else {
            None
        };

        let sign_verify_channel = self.sign_verify_requests.clone();
        let ticker_request_sender = self.ticker_requests.clone();

        if let Some((tx_type, token, address, provided_fee)) = tx_fee_info {
            let should_enforce_fee = !matches!(tx_type, TxFeeTypes::ChangePubKey { .. })
                || self.enforce_pubkey_change_fee;

            let fee_allowed =
                Self::token_allowed_for_fees(ticker_request_sender.clone(), token.clone()).await?;

            if !fee_allowed {
                return Err(SubmitError::InappropriateFeeToken);
            }

            let required_fee_data =
                Self::ticker_request(ticker_request_sender, tx_type, address, token.clone())
                    .await?;

            // Converting `BitUint` to `BigInt` is safe.
            let required_fee: BigDecimal = required_fee_data
                .normal_fee
                .total_fee
                .to_bigint()
                .unwrap()
                .into();
            let provided_fee: BigDecimal = provided_fee.to_bigint().unwrap().into();
            // Scaling the fee required since the price may change between signing the transaction and sending it to the server.
            let scaled_provided_fee = scale_user_fee_up(provided_fee.clone());
            if required_fee >= scaled_provided_fee && should_enforce_fee {
                let max_subsidy = required_fee_data.get_max_subsidy(&allowed_subsidy);

                if max_subsidy >= &required_fee - &scaled_provided_fee {
                    paid_subsidy += required_fee_data.subsidy_size_usd
                } else {
                    vlog::error!(
                        "User provided fee is too low, required: {}, provided: {} (scaled: {}); difference {}, token: {:?}",
                        required_fee.to_string(),
                        provided_fee.to_string(),
                        scaled_provided_fee.to_string(),
                        (&required_fee - &scaled_provided_fee).to_string(),
                        token
                    );

                    return Err(SubmitError::TxAdd(TxAddError::TxFeeTooLow));
                }
            }
        }

        let tx_sender = self
            .get_tx_sender(&tx)
            .await
            .or(Err(SubmitError::TxAdd(TxAddError::DbError)))?;

        let verified_tx = verify_tx_info_message_signature(
            &tx,
            tx_sender,
            token.clone(),
            self.get_tx_sender_type(&tx).await?,
            signature.tx_signature().clone(),
            msg_to_sign,
            sign_verify_channel,
        )
        .await?
        .unwrap_tx();

        if let ZkSyncTx::Swap(tx) = &tx {
            let signatures = signature.orders_signatures();
            self.verify_order_eth_signature(&tx.orders.0, signatures.0.clone())
                .await?;
            self.verify_order_eth_signature(&tx.orders.1, signatures.1.clone())
                .await?;
        }

        let tx_hash = verified_tx.tx.hash();
        // Send verified transactions to the mempool.
        self.core_api_client
            .send_tx(verified_tx)
            .await
            .map_err(SubmitError::communication_core_server)?
            .map_err(SubmitError::TxAdd)?;
        // if everything is OK, return the transactions hashes.
        if paid_subsidy > Ratio::from_integer(0u32.into()) {
            let paid_subsidy_dec = ratio_to_big_decimal(&paid_subsidy, 6).to_string();
            let total_paid_subsidy = ratio_to_big_decimal(
                &self
                    .subsidy_accumulator
                    .get_total_paid_subsidy(&token.address),
                6,
            );
            vlog::info!(
                "Paid subsidy for tx, tx: {}, token: {}, subsidy_tx: {} USD, subsidy_token_total: {} USD",
                tx_hash.to_string(),
                &token.address,
                paid_subsidy_dec,
                total_paid_subsidy
            );
            self.subsidy_accumulator
                .add_used_subsidy(&token.address, paid_subsidy);
        }
        Ok(tx.hash())
    }

    pub async fn submit_txs_batch(
        &self,
        txs: Vec<TxWithSignature>,
        eth_signatures: Option<EthBatchSignatures>,
    ) -> Result<Vec<TxHash>, SubmitError> {
        // Bring the received signatures into a vector for simplified work.
        let eth_signatures = EthBatchSignatures::api_arg_to_vec(eth_signatures);

        if txs.is_empty() {
            return Err(SubmitError::TxAdd(TxAddError::EmptyBatch));
        }
        // Even though this is going to be checked on the Mempool part,
        // we don't want to verify huge batches as long as this operation
        // is expensive.
        if txs.len() > self.max_number_of_transactions_per_batch {
            return Err(SubmitError::TxAdd(TxAddError::BatchTooBig));
        }
        // Same check but in terms of signatures.
        if eth_signatures.len() > self.max_number_of_authors_per_batch {
            return Err(SubmitError::TxAdd(TxAddError::EthSignaturesLimitExceeded));
        }

        if txs.iter().any(|tx| tx.tx.is_close()) {
            return Err(SubmitError::AccountCloseDisabled);
        }

        // Checking fees data
        let mut provided_total_usd_fee = BigDecimal::from(0);
        let mut transaction_types = vec![];

        let eth_token = TokenLike::Id(TokenId(0));

        let mut token_fees = HashMap::<Address, BigUint>::new();

        for tx in &txs {
            let tx_fee_info = tx.tx.get_fee_info();

            if let Some((tx_type, token, address, provided_fee)) = tx_fee_info {
                // Save the transaction type before moving on to the next one, otherwise
                // the total fee won't get affected by it.
                transaction_types.push((tx_type, address));

                if provided_fee == BigUint::zero() {
                    continue;
                }
                let fee_allowed =
                    Self::token_allowed_for_fees(self.ticker_requests.clone(), token.clone())
                        .await?;

                // In batches, transactions with non-popular token are allowed to be included, but should not
                // used to pay fees. Fees must be covered by some more common token.
                if !fee_allowed && provided_fee != 0u64.into() {
                    return Err(SubmitError::InappropriateFeeToken);
                }

                let check_token = if fee_allowed {
                    // For allowed tokens, we perform check in the transaction token (as expected).
                    token.clone()
                } else {
                    // For non-popular tokens we've already checked that the provided fee is 0,
                    // and the USD price will be checked in ETH.
                    eth_token.clone()
                };

                let token_price_in_usd = Self::ticker_price_request(
                    self.ticker_requests.clone(),
                    check_token.clone(),
                    TokenPriceRequestType::USDForOneWei,
                )
                .await?;

                let token_data = self.token_info_from_id(token).await?;
                let mut token_fee = token_fees.remove(&token_data.address).unwrap_or_default();
                token_fee += &provided_fee;
                token_fees.insert(token_data.address, token_fee);

                provided_total_usd_fee +=
                    BigDecimal::from(provided_fee.clone().to_bigint().unwrap())
                        * &token_price_in_usd;
            }
        }

        let mut subsidy_paid = None;
        // Only one token in batch
        if token_fees.len() == 1 {
            let (batch_token, fee_paid) = token_fees.into_iter().next().unwrap();
            let batch_token_fee = Self::ticker_batch_fee_request(
                self.ticker_requests.clone(),
                transaction_types.clone(),
                batch_token.into(),
            )
            .await?;
            let user_provided_fee =
                scale_user_fee_up(BigDecimal::from(fee_paid.to_bigint().unwrap()));
            let required_normal_fee =
                BigDecimal::from(batch_token_fee.normal_fee.total_fee.to_bigint().unwrap());

            // Not enough fee
            if required_normal_fee >= user_provided_fee {
                let allowed_subsidy = self.subsidy_accumulator.get_allowed_subsidy(&batch_token);
                let max_subsidy = batch_token_fee.get_max_subsidy(&allowed_subsidy);
                let required_subsidy = &required_normal_fee - &user_provided_fee;
                // check if subsidy can be used
                if max_subsidy >= required_subsidy {
                    subsidy_paid = Some((batch_token, batch_token_fee.subsidy_size_usd));
                } else {
                    vlog::error!(
                        "User provided batch fee in token is too low, required: {}, provided (scaled): {} (subsidy: {})",
                        required_normal_fee.to_string(),
                        user_provided_fee.to_string(),
                        max_subsidy.to_string(),
                    );
                    return Err(SubmitError::TxAdd(TxAddError::TxBatchFeeTooLow));
                }
            }
        } else {
            // Calculate required fee for ethereum token
            let required_eth_fee = Self::ticker_batch_fee_request(
                self.ticker_requests.clone(),
                transaction_types,
                eth_token.clone(),
            )
            .await?
            .normal_fee;

            let eth_price_in_usd = Self::ticker_price_request(
                self.ticker_requests.clone(),
                eth_token,
                TokenPriceRequestType::USDForOneWei,
            )
            .await?;

            let required_total_usd_fee =
                BigDecimal::from(required_eth_fee.total_fee.to_bigint().unwrap())
                    * &eth_price_in_usd;

            // Scaling the fee required since the price may change between signing the transaction and sending it to the server.
            let scaled_provided_fee_in_usd = scale_user_fee_up(provided_total_usd_fee.clone());
            if required_total_usd_fee >= scaled_provided_fee_in_usd {
                vlog::error!(
                    "User provided batch fee is too low, required: {}, provided: {} (scaled: {}); difference {}",
                    &required_total_usd_fee,
                    provided_total_usd_fee.to_string(),
                    scaled_provided_fee_in_usd.to_string(),
                    (&required_total_usd_fee - &scaled_provided_fee_in_usd).to_string(),
                );
                return Err(SubmitError::TxAdd(TxAddError::TxBatchFeeTooLow));
            }
        }

        for tx in txs.iter() {
            if let ZkSyncTx::Swap(swap) = &tx.tx {
                let signatures = tx.signature.orders_signatures();
                self.verify_order_eth_signature(&swap.orders.0, signatures.0.clone())
                    .await?;
                self.verify_order_eth_signature(&swap.orders.1, signatures.1.clone())
                    .await?;
            }
        }

        let mut verified_txs = Vec::with_capacity(txs.len());
        let mut verified_signatures = Vec::new();

        let mut messages_to_sign = Vec::with_capacity(txs.len());
        let mut tx_senders = Vec::with_capacity(txs.len());
        let mut tx_sender_types = Vec::with_capacity(txs.len());
        let mut tokens = Vec::with_capacity(txs.len());
        for tx in txs.iter().map(|tx| &tx.tx) {
            // Resolve the token and save it for constructing the batch message.
            let token = self.token_info_from_id(tx.token_id()).await?;
            tokens.push(token.clone());

            messages_to_sign.push(tx.get_ethereum_sign_message(token).map(String::into_bytes));
            tx_senders.push(
                self.get_tx_sender(tx)
                    .await
                    .or(Err(SubmitError::TxAdd(TxAddError::DbError)))?,
            );
            tx_sender_types.push(self.get_tx_sender_type(&tx).await?);
        }

        let batch_sign_data = if !eth_signatures.is_empty() {
            // User provided at least one signature for the whole batch.
            // In this case each sender cannot be CREATE2.
            if tx_sender_types
                .iter()
                .any(|_type| matches!(_type, EthAccountType::CREATE2))
            {
                return Err(SubmitError::IncorrectTx(
                    "Eth signature from CREATE2 account not expected".to_string(),
                ));
            }
            let _txs = txs
                .iter()
                .zip(tokens.iter().cloned())
                .zip(tx_senders.iter().cloned())
                .map(|((tx, token), sender)| (tx.tx.clone(), token, sender))
                .collect::<Vec<_>>();
            // Create batch signature data.
            Some(EthBatchSignData::new(_txs, eth_signatures).map_err(SubmitError::other)?)
        } else {
            None
        };
        let (verified_batch, sign_data) = verify_txs_batch_signature(
            txs,
            tx_senders,
            tokens,
            tx_sender_types,
            batch_sign_data,
            messages_to_sign,
            self.sign_verify_requests.clone(),
        )
        .await?
        .unwrap_batch();
        if let Some(sign_data) = sign_data {
            verified_signatures.extend(sign_data.signatures.into_iter());
        }
        verified_txs.extend(verified_batch.into_iter());

        if let Some((subsidy_token, subsidy_paid)) = subsidy_paid {
            let paid_subsidy_dec = ratio_to_big_decimal(&subsidy_paid, 6);
            let total_paid_subsidy = ratio_to_big_decimal(
                &self
                    .subsidy_accumulator
                    .get_total_paid_subsidy(&subsidy_token),
                6,
            );

            vlog::info!(
                "Paid subsidy for batch: token: {}, , subsidy_tx: {} USD, subsidy_token_total: {} USD",
                subsidy_token,
                paid_subsidy_dec,
                total_paid_subsidy
            );
            self.subsidy_accumulator
                .add_used_subsidy(&subsidy_token, subsidy_paid);
        }

        let tx_hashes: Vec<TxHash> = verified_txs.iter().map(|tx| tx.tx.hash()).collect();
        // Send verified transactions to the mempool.
        self.core_api_client
            .send_txs_batch(verified_txs, verified_signatures)
            .await
            .map_err(SubmitError::communication_core_server)?
            .map_err(SubmitError::TxAdd)?;

        Ok(tx_hashes)
    }

    pub async fn get_txs_fee_in_wei(
        &self,
        tx_type: TxFeeTypes,
        address: Address,
        token: TokenLike,
    ) -> Result<Fee, SubmitError> {
        let resp_fee = Self::ticker_request(
            self.ticker_requests.clone(),
            tx_type,
            address,
            token.clone(),
        )
        .await?;

        if resp_fee.subsidy_fee.total_fee == resp_fee.normal_fee.total_fee {
            return Ok(resp_fee.normal_fee);
        }

        let token = self.token_info_from_id(token).await?;

        let allowed_subsidy = self.subsidy_accumulator.get_allowed_subsidy(&token.address);
        if allowed_subsidy >= resp_fee.subsidy_size_usd {
            Ok(resp_fee.subsidy_fee)
        } else {
            Ok(resp_fee.normal_fee)
        }
    }

    pub async fn get_txs_batch_fee_in_wei(
        &self,
        transactions: Vec<(TxFeeTypes, Address)>,
        token: TokenLike,
    ) -> Result<BatchFee, SubmitError> {
        let resp_fee = Self::ticker_batch_fee_request(
            self.ticker_requests.clone(),
            transactions,
            token.clone(),
        )
        .await?;

        if resp_fee.normal_fee.total_fee == resp_fee.subsidy_fee.total_fee {
            return Ok(resp_fee.normal_fee);
        }

        let token = self.token_info_from_id(token).await?;

        let allowed_subsidy = self.subsidy_accumulator.get_allowed_subsidy(&token.address);
        if allowed_subsidy >= resp_fee.subsidy_size_usd {
            Ok(resp_fee.subsidy_fee)
        } else {
            Ok(resp_fee.normal_fee)
        }
    }

    /// For forced exits, we must check that target account exists for more
    /// than 24 hours in order to give new account owners give an opportunity
    /// to set the signing key. While `ForcedExit` operation doesn't do anything
    /// bad to the account, it's more user-friendly to only allow this operation
    /// after we're somewhat sure that zkSync account is not owned by anybody.
    async fn check_forced_exit(
        &self,
        forced_exit: &zksync_types::ForcedExit,
    ) -> Result<(), SubmitError> {
        let mut storage = self
            .pool
            .access_storage()
            .await
            .map_err(SubmitError::internal)?;

        self.forced_exit_checker
            .validate_forced_exit(&mut storage, forced_exit.target)
            .await
    }

    /// Returns a message that user has to sign to send the transaction.
    /// If the transaction doesn't need a message signature, returns `None`.
    /// If any error is encountered during the message generation, returns `jsonrpc_core::Error`.
    #[allow(dead_code)]
    async fn tx_message_to_sign(&self, tx: &ZkSyncTx) -> Result<Option<Vec<u8>>, SubmitError> {
        Ok(match tx {
            ZkSyncTx::Transfer(tx) => {
                let token = self.token_info_from_id(tx.token).await?;

                let msg = tx
                    .get_ethereum_sign_message(&token.symbol, token.decimals)
                    .into_bytes();
                Some(msg)
            }
            ZkSyncTx::Withdraw(tx) => {
                let token = self.token_info_from_id(tx.token).await?;

                let msg = tx
                    .get_ethereum_sign_message(&token.symbol, token.decimals)
                    .into_bytes();
                Some(msg)
            }

            ZkSyncTx::MintNFT(tx) => {
                let token = self.token_info_from_id(tx.fee_token).await?;

                let msg = tx
                    .get_ethereum_sign_message(&token.symbol, token.decimals)
                    .into_bytes();
                Some(msg)
            }
            _ => None,
        })
    }

    /// Resolves the token from the database.
    pub(crate) async fn token_info_from_id(
        &self,
        token_id: impl Into<TokenLike>,
    ) -> Result<Token, SubmitError> {
        let mut storage = self
            .pool
            .access_storage()
            .await
            .map_err(SubmitError::internal)?;

        self.tokens
            .get_token(&mut storage, token_id)
            .await
            .map_err(SubmitError::internal)?
            // TODO Make error more clean
            .ok_or_else(|| SubmitError::other("Token not found in the DB"))
    }

    async fn ticker_batch_fee_request(
        mut ticker_request_sender: mpsc::Sender<TickerRequest>,
        transactions: Vec<(TxFeeTypes, Address)>,
        token: TokenLike,
    ) -> Result<ResponseBatchFee, SubmitError> {
        let req = oneshot::channel();
        ticker_request_sender
            .send(TickerRequest::GetBatchTxFee {
                transactions,
                token: token.clone(),
                response: req.0,
            })
            .await
            .map_err(SubmitError::internal)?;
        let resp = req.1.await.map_err(SubmitError::internal)?;
        resp.map_err(|err| internal_error!(err))
    }

    async fn ticker_request(
        mut ticker_request_sender: mpsc::Sender<TickerRequest>,
        tx_type: TxFeeTypes,
        address: Address,
        token: TokenLike,
    ) -> Result<ResponseFee, SubmitError> {
        let req = oneshot::channel();
        ticker_request_sender
            .send(TickerRequest::GetTxFee {
                tx_type,
                address,
                token: token.clone(),
                response: req.0,
            })
            .await
            .map_err(SubmitError::internal)?;

        let resp = req.1.await.map_err(SubmitError::internal)?;
        resp.map_err(|err| internal_error!(err))
    }

    pub async fn token_allowed_for_fees(
        mut ticker_request_sender: mpsc::Sender<TickerRequest>,
        token: TokenLike,
    ) -> Result<bool, SubmitError> {
        let (sender, receiver) = oneshot::channel();
        ticker_request_sender
            .send(TickerRequest::IsTokenAllowed {
                token: token.clone(),
                response: sender,
            })
            .await
            .expect("ticker receiver dropped");
        receiver
            .await
            .expect("ticker answer sender dropped")
            .map_err(SubmitError::internal)
    }

    pub async fn ticker_price_request(
        mut ticker_request_sender: mpsc::Sender<TickerRequest>,
        token: TokenLike,
        req_type: TokenPriceRequestType,
    ) -> Result<BigDecimal, SubmitError> {
        let req = oneshot::channel();
        ticker_request_sender
            .send(TickerRequest::GetTokenPrice {
                token: token.clone(),
                response: req.0,
                req_type,
            })
            .await
            .map_err(SubmitError::internal)?;
        let resp = req.1.await.map_err(SubmitError::internal)?;
        resp.map_err(|err| internal_error!(err))
    }
}

async fn send_verify_request_and_recv(
    request: VerifySignatureRequest,
    mut req_channel: mpsc::Sender<VerifySignatureRequest>,
    receiver: oneshot::Receiver<Result<VerifiedTx, TxAddError>>,
) -> Result<VerifiedTx, SubmitError> {
    // Send the check request.
    req_channel
        .send(request)
        .await
        .map_err(SubmitError::internal)?;
    // Wait for the check result.
    receiver
        .await
        .map_err(|err| internal_error!(err))?
        .map_err(SubmitError::TxAdd)
}

/// Send a request for Ethereum signature verification and wait for the response.
/// If `msg_to_sign` is not `None`, then the signature must be present.
async fn verify_tx_info_message_signature(
    tx: &ZkSyncTx,
    tx_sender: Address,
    token: Token,
    account_type: EthAccountType,
    signature: Option<TxEthSignature>,
    msg_to_sign: Option<Vec<u8>>,
    req_channel: mpsc::Sender<VerifySignatureRequest>,
) -> Result<VerifiedTx, SubmitError> {
    let eth_sign_data = match msg_to_sign {
        Some(message) => match account_type {
            // Check if account is a CREATE2 account
            // These accounts do not have to pass 2FA
            EthAccountType::CREATE2 => {
                if signature.is_some() {
                    return Err(SubmitError::IncorrectTx(
                        "Eth signature from CREATE2 account not expected".to_string(),
                    ));
                }
                None
            }
            EthAccountType::Owned => {
                let signature =
                    signature.ok_or(SubmitError::TxAdd(TxAddError::MissingEthSignature))?;
                Some(EthSignData { signature, message })
            }
        },
        None => None,
    };

    let (sender, receiever) = oneshot::channel();

    let request = VerifySignatureRequest {
        data: RequestData::Tx(TxRequest {
            tx: SignedZkSyncTx {
                tx: tx.clone(),
                eth_sign_data,
            },
            sender: tx_sender,
            token,
        }),
        response: sender,
    };

    send_verify_request_and_recv(request, req_channel, receiever).await
}

/// Send a request for Ethereum signature verification and wait for the response.
/// Unlike in case of `verify_tx_info_message_signature`, we do not require
/// every transaction from the batch to be signed. The signature must be obtained
/// through signing a human-readable message with accordance to zkSync protocol.
async fn verify_txs_batch_signature(
    batch: Vec<TxWithSignature>,
    senders: Vec<Address>,
    tokens: Vec<Token>,
    sender_types: Vec<EthAccountType>,
    batch_sign_data: Option<EthBatchSignData>,
    msgs_to_sign: Vec<Option<Vec<u8>>>,
    req_channel: mpsc::Sender<VerifySignatureRequest>,
) -> Result<VerifiedTx, SubmitError> {
    // This hashset holds addresses that have performed a CREATE2 ChangePubKey
    // within this batch, so that we don't check ETH signatures on their transactions
    // from this batch. We save the account type to the db later.
    let mut create2_senders = HashSet::<H160>::new();
    let mut txs = Vec::with_capacity(batch.len());
    for (tx, message, sender, mut sender_type) in
        izip!(batch, msgs_to_sign, senders.iter(), sender_types)
    {
        if create2_senders.contains(sender) {
            sender_type = EthAccountType::CREATE2;
        }
        if let ZkSyncTx::ChangePubKey(tx) = &tx.tx {
            if let Some(auth_data) = &tx.eth_auth_data {
                if auth_data.is_create2() {
                    create2_senders.insert(*sender);
                }
            }
        }
        // If we have more signatures provided than required,
        // we will verify those too.
        let eth_sign_data = if let Some(message) = message {
            match sender_type {
                EthAccountType::CREATE2 => {
                    if tx.signature.exists() {
                        return Err(SubmitError::IncorrectTx(
                            "Eth signature from CREATE2 account not expected".to_string(),
                        ));
                    }
                    None
                }
                EthAccountType::Owned => {
                    if batch_sign_data.is_none() && !tx.signature.exists() {
                        return Err(SubmitError::TxAdd(TxAddError::MissingEthSignature));
                    }
                    tx.signature
                        .tx_signature()
                        .clone()
                        .map(|signature| EthSignData { signature, message })
                }
            }
        } else {
            None
        };

        txs.push(SignedZkSyncTx {
            tx: tx.tx,
            eth_sign_data,
        });
    }

    let (sender, receiver) = oneshot::channel();

    let request = VerifySignatureRequest {
        data: RequestData::Batch(BatchRequest {
            txs,
            batch_sign_data,
            senders,
            tokens,
        }),
        response: sender,
    };

    send_verify_request_and_recv(request, req_channel, receiver).await
}

/// Scales the fee provided by user up to check whether the provided fee is enough to cover our expenses for
/// maintaining the protocol.
///
/// We calculate both `provided_fee * 1.05` and `provided_fee + 1 cent` and choose the maximum.
/// This is required since the price may change between signing the transaction and sending it to the server.
fn scale_user_fee_up(provided_total_usd_fee: BigDecimal) -> BigDecimal {
    let one_cent = BigDecimal::from_str("0.01").unwrap();

    // This formula is needed when the fee is really small.
    //
    // We don't compare it with any of the following scaled numbers, because
    // a) Scaling by two (100%) is always greater than scaling by 5%.
    // b) It is intended as a smaller substitute for 1 cent scaling when
    // scaling by 1 cent means scaling more than 2x.
    if provided_total_usd_fee < one_cent {
        let scaled_by_two_provided_fee_in_usd = provided_total_usd_fee * BigDecimal::from(2u32);

        return scaled_by_two_provided_fee_in_usd;
    }

    // Scale by 5%.
    let scaled_percent_provided_fee_in_usd =
        provided_total_usd_fee.clone() * BigDecimal::from(105u32) / BigDecimal::from(100u32);

    // Scale by 1 cent.
    let scaled_one_cent_provided_fee_in_usd = provided_total_usd_fee + one_cent;

    // Choose the maximum of these two values.
    std::cmp::max(
        scaled_percent_provided_fee_in_usd,
        scaled_one_cent_provided_fee_in_usd,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scaling_user_fee_by_two() {
        let provided_fee = BigDecimal::from_str("0.005").unwrap();
        let provided_fee_scaled_by_two = BigDecimal::from_str("0.01").unwrap();

        let scaled_fee = scale_user_fee_up(provided_fee);

        assert_eq!(provided_fee_scaled_by_two, scaled_fee);
    }

    #[test]
    fn test_scaling_user_fee_by_one_cent() {
        let provided_fee = BigDecimal::from_str("0.015").unwrap();
        let provided_fee_scaled_by_cent = BigDecimal::from_str("0.025").unwrap();

        let scaled_fee = scale_user_fee_up(provided_fee);

        assert_eq!(provided_fee_scaled_by_cent, scaled_fee);
    }

    #[test]
    fn test_scaling_user_fee_by_5_percent() {
        let provided_fee = BigDecimal::from_str("0.30").unwrap();
        let provided_fee_scaled_by_five_percent = BigDecimal::from_str("0.315").unwrap();

        let scaled_fee = scale_user_fee_up(provided_fee);

        assert_eq!(provided_fee_scaled_by_five_percent, scaled_fee);
    }
}
