//! Helper module to submit transactions into the zkSync Network.

// Built-in uses
use std::{fmt::Display, str::FromStr};

// External uses
use bigdecimal::BigDecimal;
use chrono::Utc;
use futures::{
    channel::{mpsc, oneshot},
    prelude::*,
};
use num::bigint::ToBigInt;
use thiserror::Error;

// Workspace uses
use zksync_config::ApiServerOptions;
use zksync_storage::ConnectionPool;
use zksync_types::{
    tx::EthSignData,
    tx::{SignedZkSyncTx, TxEthSignature, TxHash},
    Address, Token, TokenId, TokenLike, TxFeeTypes, ZkSyncTx,
};

// Local uses
use crate::{
    core_api_client::CoreApiClient,
    fee_ticker::{Fee, TickerRequest, TokenPriceRequestType},
    signature_checker::{TxVariant, VerifiedTx, VerifyTxSignatureRequest},
    tx_error::TxAddError,
    utils::token_db_cache::TokenDBCache,
};

#[derive(Clone)]
pub struct TxSender {
    pub core_api_client: CoreApiClient,
    pub sign_verify_requests: mpsc::Sender<VerifyTxSignatureRequest>,
    pub ticker_requests: mpsc::Sender<TickerRequest>,

    pub pool: ConnectionPool,
    pub tokens: TokenDBCache,
    /// Mimimum age of the account for `ForcedExit` operations to be allowed.
    pub forced_exit_minimum_account_age: chrono::Duration,
    pub enforce_pubkey_change_fee: bool,
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
    fn internal(inner: impl Into<anyhow::Error>) -> Self {
        Self::Internal(inner.into())
    }

    fn other(msg: impl Display) -> Self {
        Self::Other(msg.to_string())
    }

    fn communication_core_server(msg: impl Display) -> Self {
        Self::CommunicationCoreServer(msg.to_string())
    }

    fn invalid_params(msg: impl Display) -> Self {
        Self::InvalidParams(msg.to_string())
    }
}

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
        sign_verify_request_sender: mpsc::Sender<VerifyTxSignatureRequest>,
        ticker_request_sender: mpsc::Sender<TickerRequest>,
        api_server_options: &ApiServerOptions,
    ) -> Self {
        let core_api_client = CoreApiClient::new(api_server_options.core_server_url.clone());

        Self::with_client(
            core_api_client,
            connection_pool,
            sign_verify_request_sender,
            ticker_request_sender,
            api_server_options,
        )
    }

    pub(crate) fn with_client(
        core_api_client: CoreApiClient,
        connection_pool: ConnectionPool,
        sign_verify_request_sender: mpsc::Sender<VerifyTxSignatureRequest>,
        ticker_request_sender: mpsc::Sender<TickerRequest>,
        api_server_options: &ApiServerOptions,
    ) -> Self {
        let enforce_pubkey_change_fee = api_server_options.enforce_pubkey_change_fee;
        let forced_exit_minimum_account_age =
            chrono::Duration::from_std(api_server_options.forced_exit_minimum_account_age)
                .expect("Unable to convert std::Duration to chrono::Duration");

        Self {
            core_api_client,
            pool: connection_pool.clone(),
            sign_verify_requests: sign_verify_request_sender,
            ticker_requests: ticker_request_sender,
            tokens: TokenDBCache::new(connection_pool),

            enforce_pubkey_change_fee,
            forced_exit_minimum_account_age,
        }
    }

    pub async fn submit_tx(
        &self,
        mut tx: ZkSyncTx,
        signature: Option<TxEthSignature>,
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

        let msg_to_sign = self.tx_message_to_sign(&tx).await?;

        let tx_fee_info = tx.get_fee_info();

        let sign_verify_channel = self.sign_verify_requests.clone();
        let ticker_request_sender = self.ticker_requests.clone();

        if let Some((tx_type, token, address, provided_fee)) = tx_fee_info {
            let should_enforce_fee =
                !matches!(tx_type, TxFeeTypes::ChangePubKey{..}) || self.enforce_pubkey_change_fee;

            let fee_allowed =
                Self::token_allowed_for_fees(ticker_request_sender.clone(), token.clone()).await?;

            if !fee_allowed {
                return Err(SubmitError::InappropriateFeeToken);
            }

            let required_fee =
                Self::ticker_request(ticker_request_sender, tx_type, address, token.clone())
                    .await?;
            // Converting `BitUint` to `BigInt` is safe.
            let required_fee: BigDecimal = required_fee.total_fee.to_bigint().unwrap().into();
            let provided_fee: BigDecimal = provided_fee.to_bigint().unwrap().into();
            // Scaling the fee required since the price may change between signing the transaction and sending it to the server.
            let scaled_provided_fee = scale_user_fee_up(provided_fee.clone());
            if required_fee >= scaled_provided_fee && should_enforce_fee {
                log::error!(
                    "User provided fee is too low, required: {}, provided: {} (scaled: {}); difference {}, token: {:?}",
                    required_fee.to_string(),
                    provided_fee.to_string(),
                    scaled_provided_fee.to_string(),
                    (required_fee - scaled_provided_fee).to_string(),
                    token
                );

                return Err(SubmitError::TxAdd(TxAddError::TxFeeTooLow));
            }
        }

        let verified_tx = verify_tx_info_message_signature(
            &tx,
            signature.clone(),
            msg_to_sign,
            sign_verify_channel,
        )
        .await?
        .unwrap_tx();

        // Send verified transactions to the mempool.
        self.core_api_client
            .send_tx(verified_tx)
            .await
            .map_err(SubmitError::communication_core_server)?
            .map_err(SubmitError::TxAdd)?;
        // if everything is OK, return the transactions hashes.
        Ok(tx.hash())
    }

    pub async fn submit_txs_batch(
        &self,
        txs: Vec<(ZkSyncTx, Option<TxEthSignature>)>,
        eth_signature: Option<TxEthSignature>,
    ) -> Result<Vec<TxHash>, SubmitError> {
        debug_assert!(txs.is_empty(), "Transaction batch cannot be empty");

        if txs.iter().any(|tx| tx.0.is_close()) {
            return Err(SubmitError::AccountCloseDisabled);
        }

        // Checking fees data
        let mut required_total_usd_fee = BigDecimal::from(0);
        let mut provided_total_usd_fee = BigDecimal::from(0);
        for tx in &txs {
            let tx_fee_info = tx.0.get_fee_info();

            if let Some((tx_type, token, address, provided_fee)) = tx_fee_info {
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
                    TokenLike::Id(0)
                };

                let required_fee = Self::ticker_request(
                    self.ticker_requests.clone(),
                    tx_type,
                    address,
                    check_token.clone(),
                )
                .await?;
                let token_price_in_usd = Self::ticker_price_request(
                    self.ticker_requests.clone(),
                    check_token.clone(),
                    TokenPriceRequestType::USDForOneWei,
                )
                .await?;

                required_total_usd_fee +=
                    BigDecimal::from(required_fee.total_fee.to_bigint().unwrap())
                        * &token_price_in_usd;
                provided_total_usd_fee +=
                    BigDecimal::from(provided_fee.clone().to_bigint().unwrap())
                        * &token_price_in_usd;
            }
        }
        // Scaling the fee required since the price may change between signing the transaction and sending it to the server.
        let scaled_provided_fee_in_usd = scale_user_fee_up(provided_total_usd_fee.clone());
        if required_total_usd_fee >= scaled_provided_fee_in_usd {
            log::error!(
                "User provided batch fee is too low, required: {}, provided: {} (scaled: {}); difference {}",
                required_total_usd_fee.to_string(),
                provided_total_usd_fee.to_string(),
                scaled_provided_fee_in_usd.to_string(),
                (required_total_usd_fee - scaled_provided_fee_in_usd).to_string(),
            );
            return Err(SubmitError::TxAdd(TxAddError::TxBatchFeeTooLow));
        }

        let mut verified_txs = Vec::new();
        let mut verified_signature = None;

        let mut messages_to_sign = vec![];
        for tx in &txs {
            messages_to_sign.push(self.tx_message_to_sign(&tx.0).await?);
        }

        if let Some(signature) = eth_signature {
            // User provided the signature for the whole batch.
            let (verified_batch, sign_data) = verify_txs_batch_signature(
                txs,
                signature,
                messages_to_sign,
                self.sign_verify_requests.clone(),
            )
            .await?
            .unwrap_batch();

            verified_signature = Some(sign_data.signature);
            verified_txs.extend(verified_batch.into_iter());
        } else {
            // Otherwise, we process every transaction in turn.
            for (tx, msg_to_sign) in txs.into_iter().zip(messages_to_sign.into_iter()) {
                let verified_tx = verify_tx_info_message_signature(
                    &tx.0,
                    tx.1.clone(),
                    msg_to_sign,
                    self.sign_verify_requests.clone(),
                )
                .await?
                .unwrap_tx();

                verified_txs.push(verified_tx);
            }
        }

        let tx_hashes: Vec<TxHash> = verified_txs.iter().map(|tx| tx.tx.hash()).collect();
        // Send verified transactions to the mempool.
        self.core_api_client
            .send_txs_batch(verified_txs, verified_signature)
            .await
            .map_err(SubmitError::communication_core_server)?
            .map_err(SubmitError::TxAdd)?;

        Ok(tx_hashes)
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

        let target_account_address = forced_exit.target;

        let account_age = storage
            .chain()
            .operations_ext_schema()
            .account_created_on(&target_account_address)
            .await
            .map_err(|err| internal_error!(err, forced_exit))?;

        match account_age {
            Some(age) if Utc::now() - age < self.forced_exit_minimum_account_age => {
                let msg = format!(
                    "Target account exists less than required minimum amount ({} hours)",
                    self.forced_exit_minimum_account_age.num_hours()
                );

                Err(SubmitError::InvalidParams(msg))
            }
            None => Err(SubmitError::invalid_params("Target account does not exist")),

            Some(..) => Ok(()),
        }
    }

    /// Returns a message that user has to sign to send the transaction.
    /// If the transaction doesn't need a message signature, returns `None`.
    /// If any error is encountered during the message generation, returns `jsonrpc_core::Error`.
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
            _ => None,
        })
    }

    async fn token_info_from_id(&self, token_id: TokenId) -> Result<Token, SubmitError> {
        self.tokens
            .get_token(token_id)
            .await
            .map_err(SubmitError::internal)?
            // TODO Make error more clean
            .ok_or_else(|| SubmitError::other("Token not found in the DB"))
    }

    async fn ticker_request(
        mut ticker_request_sender: mpsc::Sender<TickerRequest>,
        tx_type: TxFeeTypes,
        address: Address,
        token: TokenLike,
    ) -> Result<Fee, SubmitError> {
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

    async fn token_allowed_for_fees(
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

    async fn ticker_price_request(
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
    request: VerifyTxSignatureRequest,
    mut req_channel: mpsc::Sender<VerifyTxSignatureRequest>,
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
    signature: Option<TxEthSignature>,
    msg_to_sign: Option<Vec<u8>>,
    req_channel: mpsc::Sender<VerifyTxSignatureRequest>,
) -> Result<VerifiedTx, SubmitError> {
    let eth_sign_data = match msg_to_sign {
        Some(message_to_sign) => {
            let signature = signature.ok_or(SubmitError::TxAdd(TxAddError::MissingEthSignature))?;

            Some(EthSignData {
                signature,
                message: message_to_sign,
            })
        }
        None => None,
    };

    let (sender, receiever) = oneshot::channel();

    let request = VerifyTxSignatureRequest {
        tx: TxVariant::Tx(SignedZkSyncTx {
            tx: tx.clone(),
            eth_sign_data,
        }),
        response: sender,
    };

    send_verify_request_and_recv(request, req_channel, receiever).await
}

pub(crate) fn get_batch_sign_message<'a, I: Iterator<Item = &'a ZkSyncTx>>(txs: I) -> Vec<u8> {
    tiny_keccak::keccak256(
        txs.flat_map(|tx| tx.get_bytes())
            .collect::<Vec<u8>>()
            .as_slice(),
    )
    .to_vec()
}

/// Send a request for Ethereum signature verification and wait for the response.
/// Unlike in case of `verify_tx_info_message_signature`, we do not require
/// every transaction from the batch to be signed. The signature must be obtained
/// through signing hash of concatenated transactions bytes.
async fn verify_txs_batch_signature(
    batch: Vec<(ZkSyncTx, Option<TxEthSignature>)>,
    signature: TxEthSignature,
    msgs_to_sign: Vec<Option<Vec<u8>>>,
    req_channel: mpsc::Sender<VerifyTxSignatureRequest>,
) -> Result<VerifiedTx, SubmitError> {
    let mut txs = Vec::with_capacity(batch.len());
    for (tx, message) in batch.into_iter().zip(msgs_to_sign.into_iter()) {
        // If we have more signatures provided than required,
        // we will verify those too.
        let eth_sign_data = if let (Some(signature), Some(message)) = (tx.1, message) {
            Some(EthSignData { signature, message })
        } else {
            None
        };
        txs.push(SignedZkSyncTx {
            tx: tx.0,
            eth_sign_data,
        });
    }
    // User is expected to sign hash of the data of all transactions in the batch.
    let message = get_batch_sign_message(txs.iter().map(|tx| &tx.tx));
    let eth_sign_data = EthSignData { signature, message };

    let (sender, receiver) = oneshot::channel();

    let request = VerifyTxSignatureRequest {
        tx: TxVariant::Batch(txs, eth_sign_data),
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
    // Scale by 5%.
    let scaled_percent_provided_fee_in_usd =
        provided_total_usd_fee.clone() * BigDecimal::from(105u32) / BigDecimal::from(100u32);

    // Scale by 1 cent.
    let scaled_one_cent_provided_fee_in_usd =
        provided_total_usd_fee + BigDecimal::from_str("0.01").unwrap();

    // Choose the maximum of these two values.
    std::cmp::max(
        scaled_percent_provided_fee_in_usd,
        scaled_one_cent_provided_fee_in_usd,
    )
}
