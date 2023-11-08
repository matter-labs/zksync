//! Helper module to submit transactions into the zkSync Network.

// Built-in uses
use std::iter::FromIterator;
use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    str::FromStr,
};

// External uses
use bigdecimal::BigDecimal;
use chrono::{Duration, Utc};
use futures::{
    channel::{mpsc, oneshot},
    prelude::*,
};
use itertools::izip;
use num::rational::Ratio;
use num::{bigint::ToBigInt, BigUint, Zero};
use thiserror::Error;

// Workspace uses
use zksync_api_types::{
    v02::transaction::{SubmitBatchResponse, Toggle2FA, Toggle2FAResponse, TxHashSerializeWrapper},
    TxWithSignature,
};
use zksync_storage::misc::records::Subsidy;
use zksync_storage::{chain::account::records::EthAccountType, ConnectionPool};
use zksync_token_db_cache::TokenDBCache;
use zksync_types::{
    tx::{
        EthBatchSignData, EthBatchSignatures, EthSignData, Order, SignedZkSyncTx, TxEthSignature,
        TxEthSignatureVariant, TxHash,
    },
    AccountId, Address, ChainId, PubKeyHash, Token, TokenId, TokenLike, TxFeeTypes, ZkSyncTx, H160,
};
use zksync_utils::{
    big_decimal_to_ratio, biguint_to_big_decimal, ratio_to_scaled_u64, scaled_big_decimal_to_ratio,
};

// Local uses
use crate::{
    api_server::forced_exit_checker::{ForcedExitAccountAgeChecker, ForcedExitChecker},
    fee_ticker::{ResponseBatchFee, ResponseFee, TokenPriceRequestType},
    signature_checker::{
        BatchRequest, OrderRequest, RequestData, Toggle2FARequest, TxRequest, VerifiedTx,
        VerifySignatureRequest,
    },
    tx_error::Toggle2FAError,
    utils::block_details_cache::BlockDetailsCache,
};
use zksync_config::configs::api::{CommonApiConfig, TokenConfig};
use zksync_mempool::MempoolTransactionRequest;
use zksync_types::tx::error::TxAddError;

use super::rpc_server::types::RequestMetadata;
use crate::fee_ticker::{FeeTicker, PriceError};

const VALIDNESS_INTERVAL_MINUTES: i64 = 40;

#[derive(Clone)]
pub struct TxSender {
    pub mempool_tx_sender: mpsc::Sender<MempoolTransactionRequest>,
    pub sign_verify_requests: mpsc::Sender<VerifySignatureRequest>,
    pub ticker: FeeTicker,

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

    pub current_subsidy_type: String,
    pub max_subsidy_usd: Ratio<BigUint>,
    pub subsidized_ips: HashSet<String>,
    pub chain_id: ChainId,
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
    TxAdd(#[from] TxAddError),
    #[error("Chosen token is not suitable for paying fees.")]
    InappropriateFeeToken,
    // Not all TxAddErrors would apply to Toggle2FA, but
    // it is helpful to re-use IncorrectEthSignature and DbError
    #[error("Failed to toggle 2FA: {0}.")]
    Toggle2FA(#[from] Toggle2FAError),

    #[error("Communication error with the mempool: {0}.")]
    MempoolCommunication(String),
    #[error("Price error {0}")]
    PriceError(#[from] PriceError),
    #[error("Internal error.")]
    Internal(#[from] anyhow::Error),
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

    pub fn mempool_communication(msg: impl Display) -> Self {
        Self::MempoolCommunication(msg.to_string())
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
        ticker: FeeTicker,
        config: &CommonApiConfig,
        token_config: &TokenConfig,
        mempool_tx_sender: mpsc::Sender<MempoolTransactionRequest>,
        chain_id: ChainId,
    ) -> Self {
        let max_number_of_transactions_per_batch =
            config.max_number_of_transactions_per_batch as usize;
        let max_number_of_authors_per_batch = config.max_number_of_authors_per_batch as usize;

        Self {
            mempool_tx_sender,
            pool: connection_pool,
            sign_verify_requests: sign_verify_request_sender,
            ticker,
            tokens: TokenDBCache::new(token_config.invalidate_token_cache_period()),
            forced_exit_checker: ForcedExitChecker::new(
                config.forced_exit_minimum_account_age_secs,
            ),
            enforce_pubkey_change_fee: config.enforce_pubkey_change_fee,
            blocks: BlockDetailsCache::new(config.caches_size),

            fee_free_accounts: HashSet::from_iter(config.fee_free_accounts.clone()),
            max_number_of_transactions_per_batch,
            max_number_of_authors_per_batch,
            current_subsidy_type: config.subsidy_name.clone(),
            max_subsidy_usd: config.max_subsidy_usd(),
            subsidized_ips: config.subsidized_ips.clone().into_iter().collect(),
            chain_id,
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
            .map_err(|_| SubmitError::TxAdd(TxAddError::DbError))
    }

    async fn get_sender_type(&self, id: AccountId) -> Result<EthAccountType, anyhow::Error> {
        Ok(self
            .pool
            .access_storage()
            .await?
            .chain()
            .account_schema()
            .account_type_by_id(id)
            .await?
            .unwrap_or(EthAccountType::Owned))
    }

    pub async fn toggle_2fa(
        &self,
        toggle_2fa: Toggle2FA,
    ) -> Result<Toggle2FAResponse, SubmitError> {
        let account_id = toggle_2fa.account_id;
        let current_type = self
            .get_sender_type(toggle_2fa.account_id)
            .await
            .map_err(|_| SubmitError::Toggle2FA(Toggle2FAError::DbError))?;

        if matches!(current_type, EthAccountType::CREATE2) {
            return Err(SubmitError::Toggle2FA(Toggle2FAError::CREATE2));
        }

        // When 2FA is being enabled, supplied PubKeyHash is not used, so such a request
        // is not valid.
        if toggle_2fa.enable && toggle_2fa.pub_key_hash.is_some() {
            return Err(SubmitError::Toggle2FA(Toggle2FAError::UnusedPubKeyHash));
        }

        let new_type = if toggle_2fa.enable {
            EthAccountType::Owned
        } else {
            EthAccountType::No2FA(toggle_2fa.pub_key_hash)
        };

        self.verify_toggle_2fa_request_eth_signature(toggle_2fa)
            .await?;

        self.pool
            .access_storage()
            .await
            .map_err(|_| SubmitError::Toggle2FA(Toggle2FAError::DbError))?
            .chain()
            .account_schema()
            .set_account_type(account_id, new_type)
            .await
            .map_err(|_| SubmitError::Toggle2FA(Toggle2FAError::DbError))?;

        Ok(Toggle2FAResponse { success: true })
    }

    async fn verify_toggle_2fa_request_eth_signature(
        &self,
        toggle_2fa: Toggle2FA,
    ) -> Result<(), SubmitError> {
        let current_time = Utc::now();
        let request_time = toggle_2fa.timestamp;
        let validness_interval = Duration::minutes(VALIDNESS_INTERVAL_MINUTES);

        if current_time - validness_interval > request_time
            || current_time + validness_interval < request_time
        {
            return Err(SubmitError::InvalidParams(format!(
                "Timestamp differs by more than {} minutes",
                VALIDNESS_INTERVAL_MINUTES
            )));
        }

        let message = toggle_2fa.get_ethereum_sign_message().into_bytes();

        let signature = toggle_2fa.signature;
        let signer = self
            .get_address_by_id(toggle_2fa.account_id)
            .await
            .or(Err(SubmitError::TxAdd(TxAddError::DbError)))?;

        let eth_sign_data = EthSignData { signature, message };
        let (sender, receiever) = oneshot::channel();

        let request = VerifySignatureRequest {
            data: RequestData::Toggle2FA(Toggle2FARequest {
                sign_data: eth_sign_data,
                sender: signer,
            }),
            response: sender,
        };

        send_verify_request_and_recv(request, self.sign_verify_requests.clone(), receiever).await?;
        Ok(())
    }

    async fn verify_order_eth_signature(
        &self,
        order: &Order,
        signature: Option<TxEthSignature>,
    ) -> Result<(), SubmitError> {
        let signer_type = self
            .get_sender_type(order.account_id)
            .await
            .map_err(|_| SubmitError::TxAdd(TxAddError::DbError))?;
        if matches!(signer_type, EthAccountType::CREATE2) {
            return if signature.is_some() {
                Err(SubmitError::IncorrectTx(
                    "Eth signature from CREATE2 account not expected".to_string(),
                ))
            } else {
                Ok(())
            };
        }

        if matches!(signer_type, EthAccountType::No2FA(None)) {
            // We don't verify signatures for accounts with no 2FA
            return Ok(());
        }
        if let EthAccountType::No2FA(Some(unchecked_hash)) = signer_type {
            let order_pub_key_hash = PubKeyHash::from_pubkey(&order.signature.pub_key.0);
            // We don't scheck the signature only if the order was signed with the same
            // is the same as unchecked PubKey
            if order_pub_key_hash == unchecked_hash {
                return Ok(());
            }
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

    // This method is left for RPC API
    #[deprecated(note = "Use the submit_tx function instead")]
    pub async fn submit_tx_with_separate_fp(
        &self,
        mut tx: ZkSyncTx,
        signature: TxEthSignatureVariant,
        fast_processing: Option<bool>,
        extracted_request_metadata: Option<RequestMetadata>,
    ) -> Result<TxHash, SubmitError> {
        let fast_processing = fast_processing.unwrap_or(false);
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

            withdraw.fast = fast_processing;
        }

        let result = self
            .submit_tx(tx, signature, extracted_request_metadata)
            .await;

        if let Err(err) = &result {
            let err_label = match err {
                SubmitError::IncorrectTx(err) => err.clone(),
                SubmitError::TxAdd(err) => err.to_string(),
                _ => "other".to_string(),
            };
            let labels = vec![("stage", "api".to_string()), ("error", err_label)];
            metrics::increment_counter!("rejected_txs", &labels);
        }

        result
    }

    pub async fn can_subsidize(
        &self,
        new_subsidy_usd: Ratio<BigUint>,
    ) -> Result<bool, anyhow::Error> {
        let subsidized_already = self
            .pool
            .access_storage()
            .await?
            .misc_schema()
            .get_total_used_subsidy_for_type(&self.current_subsidy_type)
            .await?;
        let subsidized_already_usd = scaled_big_decimal_to_ratio(subsidized_already)?;

        let result = if self.max_subsidy_usd > subsidized_already_usd {
            &self.max_subsidy_usd - &subsidized_already_usd >= new_subsidy_usd
        } else {
            false
        };

        Ok(result)
    }

    pub async fn should_subsidize_cpk(
        &self,
        normal_fee: &BigUint,
        subsidized_fee: &BigUint,
        subsidy_size_usd: &Ratio<BigUint>,
        extracted_request_metadata: Option<RequestMetadata>,
    ) -> Result<bool, SubmitError> {
        let should_subsidize_ip = if let Some(meta) = extracted_request_metadata {
            self.subsidized_ips.contains(&meta.ip)
        } else {
            false
        };

        let result = should_subsidize_ip
            && subsidized_fee < normal_fee
            && self
                .can_subsidize(subsidy_size_usd.clone())
                .await
                .map_err(SubmitError::Internal)?;

        Ok(result)
    }

    pub async fn store_subsidy_data(
        &self,
        hash: TxHash,
        normal_fee: BigUint,
        subsidized_fee: BigUint,
        token_id: TokenId,
    ) -> Result<(), anyhow::Error> {
        let token_price_in_usd = self
            .ticker
            .get_token_price(TokenLike::Id(token_id), TokenPriceRequestType::USDForOneWei)
            .await?;

        let full_cost_usd = big_decimal_to_ratio(&token_price_in_usd)? * &normal_fee;
        let subsidized_cost_usd = big_decimal_to_ratio(&token_price_in_usd)? * &subsidized_fee;
        if full_cost_usd < subsidized_cost_usd {
            return Err(anyhow::Error::msg(
                "Trying to subsidize transaction which should not be subsidized",
            ));
        }

        let subsidy = Subsidy {
            usd_amount_scaled: ratio_to_scaled_u64(&full_cost_usd - &subsidized_cost_usd),
            full_cost_usd_scaled: ratio_to_scaled_u64(full_cost_usd),
            token_id,
            token_amount: biguint_to_big_decimal(subsidized_fee),
            full_cost_token: biguint_to_big_decimal(normal_fee),
            subsidy_type: self.current_subsidy_type.clone(),
            tx_hash: hash,
        };

        self.pool
            .access_storage()
            .await?
            .misc_schema()
            .store_subsidy(subsidy)
            .await?;

        Ok(())
    }

    pub async fn submit_tx(
        &self,
        mut tx: ZkSyncTx,
        signature: TxEthSignatureVariant,
        extracted_request_metadata: Option<RequestMetadata>,
    ) -> Result<TxHash, SubmitError> {
        let labels = vec![
            ("stage", "api".to_string()),
            ("name", tx.variance_name()),
            ("token", tx.token_id().to_string()),
        ];
        // The initial state of processing tx
        metrics::increment_counter!("process_tx_count", &labels);

        if tx.is_close() {
            return Err(SubmitError::AccountCloseDisabled);
        }

        if let ZkSyncTx::ForcedExit(forced_exit) = &tx {
            self.check_forced_exit(forced_exit).await?;
        }
        if let ZkSyncTx::ChangePubKey(change_pub_key) = &mut tx {
            change_pub_key.chain_id = Some(self.chain_id)
        };

        // Resolve the token.
        let token = self.token_info_from_id(tx.token_id()).await?;
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

        let mut fee_data_for_subsidy: Option<ResponseFee> = None;

        if let Some((tx_type, token, address, provided_fee)) = tx_fee_info {
            let should_enforce_fee = !matches!(tx_type, TxFeeTypes::ChangePubKey { .. })
                || self.enforce_pubkey_change_fee;

            let fee_allowed = self.ticker.token_allowed_for_fees(token.clone()).await?;

            if !fee_allowed {
                return Err(SubmitError::InappropriateFeeToken);
            }

            let required_fee_data = self
                .ticker
                .get_fee_from_ticker_in_wei(tx_type, token.clone(), address)
                .await?;

            let required_fee_data = if self
                .should_subsidize_cpk(
                    &required_fee_data.normal_fee.total_fee,
                    &required_fee_data.subsidized_fee.total_fee,
                    &required_fee_data.subsidy_size_usd,
                    extracted_request_metadata,
                )
                .await?
            {
                fee_data_for_subsidy = Some(required_fee_data.clone());
                required_fee_data.subsidized_fee
            } else {
                required_fee_data.normal_fee
            };

            // Converting `BitUint` to `BigInt` is safe.
            let required_fee: BigDecimal = required_fee_data.total_fee.to_bigint().unwrap().into();
            let provided_fee: BigDecimal = provided_fee.to_bigint().unwrap().into();
            // Scaling the fee required since the price may change between signing the transaction and sending it to the server.
            let scaled_provided_fee = scale_user_fee_up(provided_fee);
            if required_fee >= scaled_provided_fee && should_enforce_fee {
                return Err(SubmitError::TxAdd(TxAddError::TxFeeTooLow));
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
            if signature.is_single() {
                return Err(SubmitError::TxAdd(TxAddError::MissingEthSignature));
            }
            let signatures = signature.orders_signatures();
            self.verify_order_eth_signature(&tx.orders.0, signatures.0.clone())
                .await?;
            self.verify_order_eth_signature(&tx.orders.1, signatures.1.clone())
                .await?;
        }

        let (sender, receiver) = oneshot::channel();
        let item = MempoolTransactionRequest::NewTx(Box::new(verified_tx), sender);
        let mut mempool_sender = self.mempool_tx_sender.clone();
        mempool_sender
            .send(item)
            .await
            .map_err(SubmitError::internal)?;

        receiver.await.map_err(SubmitError::internal)??;

        // fee_data_for_subsidy has Some value only if the batch of transactions is subsidised
        if let Some(fee_data_for_subsidy) = fee_data_for_subsidy {
            // The following two bad scenarios are possible when applying subsidy for the tx:
            // - The subsidy is stored, but the tx is then rejected by the state keeper
            // - The tx is accepted by the state keeper, but the the `store_subsidy_data` returns an error for some reason
            //
            // Trying to omit these scenarios unfortunately leads to large code restructure
            // which is not worth it for subsidies (we prefer stability here)
            self.store_subsidy_data(
                tx.hash(),
                fee_data_for_subsidy.normal_fee.total_fee,
                fee_data_for_subsidy.subsidized_fee.total_fee,
                token.id,
            )
            .await
            .map_err(|e| {
                metrics::increment_counter!("tx_sender.submit_tx.store_subsidy_data_fail");
                SubmitError::Other(format!(
                    "Failed to store the subsidy to database. Reason: {}",
                    e
                ))
            })?;
        }

        // if everything is OK, return the transactions hashes.
        Ok(tx.hash())
    }

    pub async fn submit_txs_batch(
        &self,
        txs: Vec<TxWithSignature>,
        eth_signatures: Option<EthBatchSignatures>,
        extracted_request_metadata: Option<RequestMetadata>,
    ) -> Result<SubmitBatchResponse, SubmitError> {
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

        for tx in &txs {
            let labels = vec![
                ("stage", "api".to_string()),
                ("name", tx.tx.variance_name()),
                ("token", tx.tx.token_id().to_string()),
            ];
            metrics::increment_counter!("process_tx_count", &labels);
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
        let mut token_fees_ids = vec![];

        for tx in &txs {
            let tx_fee_info = tx.tx.get_fee_info();

            if let Some((tx_type, token, address, provided_fee)) = tx_fee_info {
                // Save the transaction type before moving on to the next one, otherwise
                // the total fee won't get affected by it.
                transaction_types.push((tx_type, address));

                if provided_fee == BigUint::zero() {
                    continue;
                }
                let fee_allowed = self.ticker.token_allowed_for_fees(token.clone()).await?;

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

                let token_price_in_usd = self
                    .ticker
                    .get_token_price(check_token.clone(), TokenPriceRequestType::USDForOneWei)
                    .await?;

                let token_data = self.token_info_from_id(token).await?;
                token_fees_ids.push(token_data.id);
                let mut token_fee = token_fees.remove(&token_data.address).unwrap_or_default();
                token_fee += &provided_fee;
                token_fees.insert(token_data.address, token_fee);

                provided_total_usd_fee +=
                    BigDecimal::from(provided_fee.clone().to_bigint().unwrap())
                        * &token_price_in_usd;
            }
        }

        let mut fee_data_for_subsidy: Option<ResponseBatchFee> = None;

        // Only one token in batch
        if token_fees.len() == 1 {
            let (batch_token, fee_paid) = token_fees.into_iter().next().unwrap();
            let batch_token_fee = self
                .ticker
                .get_batch_from_ticker_in_wei(batch_token.into(), transaction_types.clone())
                .await?;

            let required_fee = if self
                .should_subsidize_cpk(
                    &batch_token_fee.normal_fee.total_fee,
                    &batch_token_fee.subsidized_fee.total_fee,
                    &batch_token_fee.subsidy_size_usd,
                    extracted_request_metadata,
                )
                .await?
            {
                fee_data_for_subsidy = Some(batch_token_fee.clone());
                batch_token_fee.subsidized_fee.total_fee
            } else {
                batch_token_fee.normal_fee.total_fee
            };

            let user_provided_fee =
                scale_user_fee_up(BigDecimal::from(fee_paid.to_bigint().unwrap()));
            let required_normal_fee = BigDecimal::from(required_fee.to_bigint().unwrap());

            // Not enough fee
            if required_normal_fee > user_provided_fee {
                vlog::info!(
                    "User provided batch fee in token is too low, required: {}, provided (scaled): {}",
                    required_normal_fee.to_string(),
                    user_provided_fee.to_string(),
                );
                return Err(SubmitError::TxAdd(TxAddError::TxBatchFeeTooLow));
            }
        } else {
            // Calculate required fee for ethereum token
            let required_eth_fee = self
                .ticker
                .get_batch_from_ticker_in_wei(eth_token.clone(), transaction_types)
                .await?;

            let required_fee = if self
                .should_subsidize_cpk(
                    &required_eth_fee.normal_fee.total_fee,
                    &required_eth_fee.subsidized_fee.total_fee,
                    &required_eth_fee.subsidy_size_usd,
                    extracted_request_metadata,
                )
                .await?
            {
                fee_data_for_subsidy = Some(required_eth_fee.clone());
                required_eth_fee.subsidized_fee.total_fee
            } else {
                required_eth_fee.normal_fee.total_fee
            };

            let eth_price_in_usd = self
                .ticker
                .get_token_price(eth_token, TokenPriceRequestType::USDForOneWei)
                .await?;

            let required_total_usd_fee =
                BigDecimal::from(required_fee.to_bigint().unwrap()) * &eth_price_in_usd;

            // Scaling the fee required since the price may change between signing the transaction and sending it to the server.
            let scaled_provided_fee_in_usd = scale_user_fee_up(provided_total_usd_fee.clone());
            if required_total_usd_fee > scaled_provided_fee_in_usd {
                vlog::info!(
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
                if tx.signature.is_single() {
                    return Err(SubmitError::TxAdd(TxAddError::MissingEthSignature));
                }
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
            tx_sender_types.push(self.get_tx_sender_type(tx).await?);
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

        let tx_hashes: Vec<TxHash> = verified_txs.iter().map(|tx| tx.tx.hash()).collect();

        let (sender, receiver) = oneshot::channel();
        let item =
            MempoolTransactionRequest::NewTxsBatch(verified_txs, verified_signatures, sender);
        let mut mempool_sender = self.mempool_tx_sender.clone();
        mempool_sender
            .send(item)
            .await
            .map_err(SubmitError::mempool_communication)?;

        receiver.await.map_err(SubmitError::internal)??;

        let batch_hash = TxHash::batch_hash(&tx_hashes);

        // fee_data_for_subsidy has Some value only if the batch of transactions is subsidised
        if let Some(fee_data) = fee_data_for_subsidy {
            let subsidy_token_id = if token_fees_ids.len() == 1 {
                token_fees_ids[0]
            } else {
                // When there are more than token to pay the fee with,
                // we get the price of the batch in ETH and then convert it to USD.
                // Since the `subsidies` table contains the token_id field and the only fee which is fetched from the fee_ticker is
                // in ETH, then we can consider ETH as the token_id of the subsidy. Even though formally this may not be the case.
                TokenId(0)
            };

            // The following two bad scenarios are possible when applying subsidy for the tx:
            // - The subsidy is stored, but the tx is then rejected by the state keeper
            // - The tx is accepted by the state keeper, but the the `store_subsidy_data` returns an error for some reason
            //
            // Trying to omit these scenarios unfortunately leads to large code restructure
            // which is not worth it for subsidies (we prefer stability here)
            self.store_subsidy_data(
                batch_hash,
                fee_data.normal_fee.total_fee,
                fee_data.subsidized_fee.total_fee,
                subsidy_token_id,
            )
            .await
            .map_err(|e| {
                metrics::increment_counter!("tx_sender.submit_txs_batch.store_subsidy_data_fail");

                SubmitError::Other(format!(
                    "Failed to store the subsidy to database. Reason: {}",
                    e
                ))
            })?;
        }

        Ok(SubmitBatchResponse {
            transaction_hashes: tx_hashes.into_iter().map(TxHashSerializeWrapper).collect(),
            batch_hash,
        })
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
        let token_id = token_id.into();
        // Try to find the token in the cache first.
        if let Some(token) = self.tokens.try_get_token_from_cache(token_id.clone()).await {
            return Ok(token);
        }

        // Establish db connection and repeat the query, so the token is loaded
        // from the db.
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
    if matches!(
        (account_type, signature.clone(), msg_to_sign.clone()),
        (EthAccountType::CREATE2, Some(_), Some(_))
    ) {
        return Err(SubmitError::IncorrectTx(
            "Eth signature from CREATE2 account not expected".to_string(),
        ));
    }

    let should_check_eth_signature = match (account_type, tx) {
        (EthAccountType::CREATE2, _) => false,
        (EthAccountType::No2FA(_), ZkSyncTx::ChangePubKey(_)) => true,
        (EthAccountType::No2FA(hash), _) => {
            if let Some(not_checked_hash) = hash {
                let tx_pub_key_hash = PubKeyHash::from_pubkey(&tx.signature().pub_key.0);

                tx_pub_key_hash != not_checked_hash
            } else {
                false
            }
        }

        _ => true,
    };

    let eth_sign_data = match (msg_to_sign, should_check_eth_signature) {
        (Some(message), true) => {
            let signature = signature.ok_or(SubmitError::TxAdd(TxAddError::MissingEthSignature))?;
            Some(EthSignData { signature, message })
        }
        _ => None,
    };

    let (sender, receiever) = oneshot::channel();

    let request = VerifySignatureRequest {
        data: RequestData::Tx(TxRequest {
            tx: SignedZkSyncTx {
                tx: tx.clone(),
                eth_sign_data,
                created_at: Utc::now(),
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
                EthAccountType::No2FA(Some(unchecked_hash)) => {
                    let tx_pub_key_hash = PubKeyHash::from_pubkey(&tx.tx.signature().pub_key.0);
                    if tx_pub_key_hash != unchecked_hash {
                        if batch_sign_data.is_none() && !tx.signature.exists() {
                            return Err(SubmitError::TxAdd(TxAddError::MissingEthSignature));
                        }

                        tx.signature
                            .tx_signature()
                            .clone()
                            .map(|signature| EthSignData { signature, message })
                    } else {
                        None
                    }
                }
                EthAccountType::No2FA(None) => None,
            }
        } else {
            None
        };

        txs.push(SignedZkSyncTx {
            tx: tx.tx,
            eth_sign_data,
            created_at: Utc::now(),
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
