//! Helper module to submit transactions into the zkSync Network.

// Built-in uses
use std::{collections::HashSet, fmt::Display, str::FromStr};

// External uses
use bigdecimal::BigDecimal;
use chrono::Utc;
use futures::{
    channel::{mpsc, oneshot},
    prelude::*,
};
use itertools::izip;
use num::{bigint::ToBigInt, BigUint, Zero};
use thiserror::Error;

// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_storage::{chain::account::records::EthAccountType, ConnectionPool};
use zksync_types::{
    tx::{
        EthBatchSignData, EthBatchSignatures, EthSignData, SignedZkSyncTx, TxEthSignature, TxHash,
    },
    Address, BatchFee, Fee, Token, TokenId, TokenLike, TxFeeTypes, ZkSyncTx, H160,
};

// Local uses
use crate::api_server::rpc_server::types::TxWithSignature;
use crate::{
    core_api_client::CoreApiClient,
    fee_ticker::{TickerRequest, TokenPriceRequestType},
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
    // Limit the number of both transactions and Ethereum signatures per batch.
    pub max_number_of_transactions_per_batch: usize,
    pub max_number_of_authors_per_batch: usize,
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
        sign_verify_request_sender: mpsc::Sender<VerifyTxSignatureRequest>,
        ticker_request_sender: mpsc::Sender<TickerRequest>,
        config: &ZkSyncConfig,
    ) -> Self {
        let forced_exit_minimum_account_age = chrono::Duration::seconds(
            config.api.common.forced_exit_minimum_account_age_secs as i64,
        );

        let max_number_of_transactions_per_batch =
            config.api.common.max_number_of_transactions_per_batch as usize;
        let max_number_of_authors_per_batch =
            config.api.common.max_number_of_authors_per_batch as usize;

        Self {
            core_api_client,
            pool: connection_pool,
            sign_verify_requests: sign_verify_request_sender,
            ticker_requests: ticker_request_sender,
            tokens: TokenDBCache::new(),

            enforce_pubkey_change_fee: config.api.common.enforce_pubkey_change_fee,
            forced_exit_minimum_account_age,
            max_number_of_transactions_per_batch,
            max_number_of_authors_per_batch,
        }
    }

    /// If `ForcedExit` has Ethereum siganture (e.g. it's a part of a batch), an actual signer
    /// is initiator, not the target, thus, this function will perform a database query to acquire
    /// the corresponding address.
    async fn get_tx_sender(&self, tx: &ZkSyncTx) -> Result<Address, anyhow::Error> {
        match tx {
            ZkSyncTx::ForcedExit(tx) => self
                .pool
                .access_storage()
                .await?
                .chain()
                .account_schema()
                .account_address_by_id(tx.initiator_account_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Forced Exit account is not found in db")),
            _ => Ok(tx.account()),
        }
    }

    async fn get_tx_sender_type(&self, tx: &ZkSyncTx) -> Result<EthAccountType, SubmitError> {
        Ok(self
            .pool
            .access_storage()
            .await
            .map_err(|_| SubmitError::TxAdd(TxAddError::DbError))?
            .chain()
            .account_schema()
            .account_type_by_id(tx.account_id().or(Err(SubmitError::AccountCloseDisabled))?)
            .await
            .map_err(|_| SubmitError::TxAdd(TxAddError::DbError))?
            .unwrap_or(EthAccountType::Owned))
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

        // Resolve the token.
        let token = self.token_info_from_id(tx.token_id()).await?;
        let msg_to_sign = tx
            .get_ethereum_sign_message(token.clone())
            .map(String::into_bytes);

        let tx_fee_info = tx.get_fee_info();

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

            let required_fee =
                Self::ticker_request(ticker_request_sender, tx_type, address, token.clone())
                    .await?;
            // Converting `BitUint` to `BigInt` is safe.
            let required_fee: BigDecimal = required_fee.total_fee.to_bigint().unwrap().into();
            let provided_fee: BigDecimal = provided_fee.to_bigint().unwrap().into();
            // Scaling the fee required since the price may change between signing the transaction and sending it to the server.
            let scaled_provided_fee = scale_user_fee_up(provided_fee.clone());
            if required_fee >= scaled_provided_fee && should_enforce_fee {
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

        let tx_sender = self
            .get_tx_sender(&tx)
            .await
            .or(Err(SubmitError::TxAdd(TxAddError::DbError)))?;

        let verified_tx = verify_tx_info_message_signature(
            &tx,
            tx_sender,
            token,
            self.get_tx_sender_type(&tx).await?,
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

        for tx in &txs {
            let tx_fee_info = tx.tx.get_fee_info();

            if let Some((tx_type, token, address, provided_fee)) = tx_fee_info {
                if provided_fee == BigUint::zero() {
                    continue;
                }
                let fee_allowed =
                    Self::token_allowed_for_fees(self.ticker_requests.clone(), token.clone())
                        .await?;

                transaction_types.push((tx_type, address));

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

                provided_total_usd_fee +=
                    BigDecimal::from(provided_fee.clone().to_bigint().unwrap())
                        * &token_price_in_usd;
            }
        }

        // Calculate required fee for ethereum token
        let required_eth_fee = Self::ticker_batch_fee_request(
            self.ticker_requests.clone(),
            transaction_types,
            eth_token.clone(),
        )
        .await?;

        let eth_price_in_usd = Self::ticker_price_request(
            self.ticker_requests.clone(),
            eth_token,
            TokenPriceRequestType::USDForOneWei,
        )
        .await?;

        let required_total_usd_fee =
            BigDecimal::from(required_eth_fee.total_fee.to_bigint().unwrap()) * &eth_price_in_usd;

        // Scaling the fee required since the price may change between signing the transaction and sending it to the server.
        let scaled_provided_fee_in_usd = scale_user_fee_up(provided_total_usd_fee.clone());
        if required_total_usd_fee >= scaled_provided_fee_in_usd {
            vlog::error!(
                "User provided batch fee is too low, required: {}, provided: {} (scaled: {}); difference {}",
                required_total_usd_fee.to_string(),
                provided_total_usd_fee.to_string(),
                scaled_provided_fee_in_usd.to_string(),
                (required_total_usd_fee.clone() - scaled_provided_fee_in_usd.clone()).to_string(),
            );
            return Err(SubmitError::TxAdd(TxAddError::TxBatchFeeTooLow));
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

        if !eth_signatures.is_empty() {
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
            let batch_sign_data =
                EthBatchSignData::new(_txs, eth_signatures).map_err(SubmitError::other)?;
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

            verified_signatures.extend(sign_data.signatures.into_iter());
            verified_txs.extend(verified_batch.into_iter());
        } else {
            // Otherwise, we process every transaction in turn.

            // This hashset holds addresses that have performed a CREATE2 ChangePubKey
            // within this batch, so that we don't check ETH signatures on their transactions
            // from this batch. We save the account type to the db later.
            let mut create2_senders = HashSet::<H160>::new();

            for (tx, sender, token, mut sender_type, msg_to_sign) in
                izip!(txs, tx_senders, tokens, tx_sender_types, messages_to_sign)
            {
                if create2_senders.contains(&sender) {
                    sender_type = EthAccountType::CREATE2;
                }
                let verified_tx = verify_tx_info_message_signature(
                    &tx.tx,
                    sender,
                    token,
                    sender_type,
                    tx.signature.clone(),
                    msg_to_sign,
                    self.sign_verify_requests.clone(),
                )
                .await?
                .unwrap_tx();

                if let ZkSyncTx::ChangePubKey(tx) = tx.tx {
                    if let Some(auth_data) = tx.eth_auth_data {
                        if auth_data.is_create2() {
                            create2_senders.insert(sender);
                        }
                    }
                }

                verified_txs.push(verified_tx);
            }
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
        Self::ticker_request(self.ticker_requests.clone(), tx_type, address, token).await
    }

    pub async fn get_txs_batch_fee_in_wei(
        &self,
        transactions: Vec<(TxFeeTypes, Address)>,
        token: TokenLike,
    ) -> Result<BatchFee, SubmitError> {
        Self::ticker_batch_fee_request(self.ticker_requests.clone(), transactions, token).await
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
            _ => None,
        })
    }

    /// Resolves the token from the database.
    async fn token_info_from_id(&self, token_id: TokenId) -> Result<Token, SubmitError> {
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
    ) -> Result<BatchFee, SubmitError> {
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
    tx_sender: Address,
    token: Token,
    account_type: EthAccountType,
    signature: Option<TxEthSignature>,
    msg_to_sign: Option<Vec<u8>>,
    req_channel: mpsc::Sender<VerifyTxSignatureRequest>,
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

    let request = VerifyTxSignatureRequest {
        tx: TxVariant::Tx(SignedZkSyncTx {
            tx: tx.clone(),
            eth_sign_data,
        }),
        senders: vec![tx_sender],
        tokens: vec![token],
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
    batch_sign_data: EthBatchSignData,
    msgs_to_sign: Vec<Option<Vec<u8>>>,
    req_channel: mpsc::Sender<VerifyTxSignatureRequest>,
) -> Result<VerifiedTx, SubmitError> {
    let mut txs = Vec::with_capacity(batch.len());
    for (tx, message, sender_type) in izip!(batch, msgs_to_sign, sender_types) {
        // If we have more signatures provided than required,
        // we will verify those too.
        let eth_sign_data = if let (Some(signature), Some(message)) = (tx.signature, message) {
            if let EthAccountType::CREATE2 = sender_type {
                return Err(SubmitError::IncorrectTx(
                    "Eth signature from CREATE2 account not expected".to_string(),
                ));
            }
            Some(EthSignData { signature, message })
        } else {
            None
        };
        txs.push(SignedZkSyncTx {
            tx: tx.tx,
            eth_sign_data,
        });
    }

    let (sender, receiver) = oneshot::channel();

    let request = VerifyTxSignatureRequest {
        tx: TxVariant::Batch(txs, batch_sign_data),
        senders,
        tokens,
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
