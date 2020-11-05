// External uses
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use jsonrpc_core::{Error, IoHandler, MetaIoHandler, Metadata, Middleware, Result};
use jsonrpc_http_server::ServerBuilder;
// Workspace uses
use zksync_config::ConfigurationOptions;
use zksync_storage::{
    chain::{
        block::records::BlockDetails, operations::records::StoredExecutedPriorityOperation,
        operations_ext::records::TxReceiptResponse,
    },
    ConnectionPool, StorageProcessor,
};
use zksync_types::{
    tx::{TxEthSignature, TxHash},
    Address, PriorityOp, Token, TokenId, TokenLike, TxFeeTypes, ZkSyncTx,
};
// Local uses
use crate::{
    core_api_client::{CoreApiClient, EthBlockId},
    fee_ticker::{Fee, TickerRequest, TokenPriceRequestType},
    signature_checker::{VerifiedTx, VerifyTxSignatureRequest},
    tx_error::TxAddError,
    utils::{shared_lru_cache::SharedLruCache, token_db_cache::TokenDBCache},
};
use bigdecimal::BigDecimal;
use zksync_types::tx::EthSignData;
use zksync_utils::panic_notify::ThreadPanicNotify;

pub mod error;
mod rpc_impl;
mod rpc_trait;
pub mod types;

use self::error::*;
pub use self::rpc_trait::Rpc;
use self::types::*;

pub(crate) async fn get_ongoing_priority_ops(
    api_client: &CoreApiClient,
    address: Address,
) -> Result<Vec<(EthBlockId, PriorityOp)>> {
    api_client
        .get_unconfirmed_deposits(address)
        .await
        .map_err(|_| Error::internal_error())
}

#[derive(Clone)]
pub struct RpcApp {
    runtime_handle: tokio::runtime::Handle,

    cache_of_executed_priority_operations: SharedLruCache<u32, StoredExecutedPriorityOperation>,
    cache_of_blocks_info: SharedLruCache<i64, BlockDetails>,
    cache_of_transaction_receipts: SharedLruCache<Vec<u8>, TxReceiptResponse>,
    cache_of_complete_withdrawal_tx_hashes: SharedLruCache<TxHash, String>,

    pub api_client: CoreApiClient,
    pub sign_verify_request_sender: mpsc::Sender<VerifyTxSignatureRequest>,
    pub ticker_request_sender: mpsc::Sender<TickerRequest>,

    pub connection_pool: ConnectionPool,

    pub confirmations_for_eth_event: u64,
    pub token_cache: TokenDBCache,

    /// Mimimum age of the account for `ForcedExit` operations to be allowed.
    forced_exit_minimum_account_age: chrono::Duration,
    enforce_pubkey_change_fee: bool,
}

impl RpcApp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config_options: &ConfigurationOptions,
        connection_pool: ConnectionPool,
        sign_verify_request_sender: mpsc::Sender<VerifyTxSignatureRequest>,
        ticker_request_sender: mpsc::Sender<TickerRequest>,
    ) -> Self {
        let runtime_handle = tokio::runtime::Handle::try_current()
            .expect("RpcApp must be created from the context of Tokio Runtime");

        let token_cache = TokenDBCache::new(connection_pool.clone());

        let api_client = CoreApiClient::new(config_options.core_server_url.clone());

        let api_requests_caches_size = config_options.api_requests_caches_size;
        let confirmations_for_eth_event = config_options.confirmations_for_eth_event;
        let enforce_pubkey_change_fee = config_options.enforce_pubkey_change_fee;

        let forced_exit_minimum_account_age =
            chrono::Duration::from_std(config_options.forced_exit_minimum_account_age)
                .expect("Unable to convert std::Duration to chrono::Duration");

        RpcApp {
            runtime_handle,

            cache_of_executed_priority_operations: SharedLruCache::new(api_requests_caches_size),
            cache_of_blocks_info: SharedLruCache::new(api_requests_caches_size),
            cache_of_transaction_receipts: SharedLruCache::new(api_requests_caches_size),
            cache_of_complete_withdrawal_tx_hashes: SharedLruCache::new(api_requests_caches_size),

            connection_pool,
            api_client,

            sign_verify_request_sender,
            ticker_request_sender,

            confirmations_for_eth_event,
            token_cache,

            forced_exit_minimum_account_age,
            enforce_pubkey_change_fee,
        }
    }

    pub fn extend<T: Metadata, S: Middleware<T>>(self, io: &mut MetaIoHandler<T, S>) {
        io.extend_with(self.to_delegate())
    }

    async fn token_info_from_id(&self, token_id: TokenId) -> Result<Token> {
        fn rpc_message(error: impl ToString) -> Error {
            Error {
                code: RpcErrorCodes::Other.into(),
                message: error.to_string(),
                data: None,
            }
        }

        self.token_cache
            .get_token(token_id)
            .await
            .map_err(rpc_message)?
            .ok_or_else(|| rpc_message("Token not found in the DB"))
    }

    /// Returns a message that user has to sign to send the transaction.
    /// If the transaction doesn't need a message signature, returns `None`.
    /// If any error is encountered during the message generation, returns `jsonrpc_core::Error`.
    async fn get_tx_info_message_to_sign(&self, tx: &ZkSyncTx) -> Result<Option<String>> {
        match tx {
            ZkSyncTx::Transfer(tx) => {
                let token = self.token_info_from_id(tx.token).await?;
                Ok(Some(
                    tx.get_ethereum_sign_message(&token.symbol, token.decimals),
                ))
            }
            ZkSyncTx::Withdraw(tx) => {
                let token = self.token_info_from_id(tx.token).await?;
                Ok(Some(
                    tx.get_ethereum_sign_message(&token.symbol, token.decimals),
                ))
            }
            _ => Ok(None),
        }
    }
}

impl RpcApp {
    async fn access_storage(&self) -> Result<StorageProcessor<'_>> {
        self.connection_pool
            .access_storage()
            .await
            .map_err(|_| Error::internal_error())
    }

    /// Async version of `get_ongoing_deposits` which does not use old futures as a return type.
    async fn get_ongoing_deposits_impl(&self, address: Address) -> Result<OngoingDepositsResp> {
        let confirmations_for_eth_event = self.confirmations_for_eth_event;

        let ongoing_ops = get_ongoing_priority_ops(&self.api_client, address).await?;

        let mut max_block_number = 0;

        // Transform operations into `OngoingDeposit` and find the maximum block number in a
        // single pass.
        let deposits: Vec<_> = ongoing_ops
            .into_iter()
            .map(|(block, op)| {
                if block > max_block_number {
                    max_block_number = block;
                }

                OngoingDeposit::new(block, op)
            })
            .collect();

        let estimated_deposits_approval_block = if !deposits.is_empty() {
            // We have to wait `confirmations_for_eth_event` blocks after the most
            // recent deposit operation.
            Some(max_block_number + confirmations_for_eth_event)
        } else {
            // No ongoing deposits => no estimated block.
            None
        };

        Ok(OngoingDepositsResp {
            address,
            deposits,
            confirmations_for_eth_event,
            estimated_deposits_approval_block,
        })
    }

    // cache access functions
    async fn get_executed_priority_operation(
        &self,
        serial_id: u32,
    ) -> Result<Option<StoredExecutedPriorityOperation>> {
        let res =
            if let Some(executed_op) = self.cache_of_executed_priority_operations.get(&serial_id) {
                Some(executed_op)
            } else {
                let mut storage = self.access_storage().await?;
                let executed_op = storage
                    .chain()
                    .operations_schema()
                    .get_executed_priority_operation(serial_id)
                    .await
                    .map_err(|err| {
                        log::warn!(
                            "[{}:{}:{}] Internal Server Error: '{}'; input: {}",
                            file!(),
                            line!(),
                            column!(),
                            err,
                            serial_id,
                        );
                        Error::internal_error()
                    })?;

                if let Some(executed_op) = executed_op.clone() {
                    self.cache_of_executed_priority_operations
                        .insert(serial_id, executed_op);
                }

                executed_op
            };
        Ok(res)
    }

    async fn get_block_info(&self, block_number: i64) -> Result<Option<BlockDetails>> {
        let res = if let Some(block) = self.cache_of_blocks_info.get(&block_number) {
            Some(block)
        } else {
            let mut storage = self.access_storage().await?;
            let block = storage
                .chain()
                .block_schema()
                .find_block_by_height_or_hash(block_number.to_string())
                .await;

            if let Some(block) = block.clone() {
                // Unverified blocks can still change, so we can't cache them.
                if block.verified_at.is_some() && block.block_number == block_number {
                    self.cache_of_blocks_info.insert(block_number, block);
                }
            }

            block
        };
        Ok(res)
    }

    async fn get_tx_receipt(&self, tx_hash: TxHash) -> Result<Option<TxReceiptResponse>> {
        let res = if let Some(tx_receipt) = self
            .cache_of_transaction_receipts
            .get(&tx_hash.as_ref().to_vec())
        {
            Some(tx_receipt)
        } else {
            let mut storage = self.access_storage().await?;
            let tx_receipt = storage
                .chain()
                .operations_ext_schema()
                .tx_receipt(tx_hash.as_ref())
                .await
                .map_err(|err| {
                    log::warn!(
                        "[{}:{}:{}] Internal Server Error: '{}'; input: {}",
                        file!(),
                        line!(),
                        column!(),
                        err,
                        tx_hash.to_string(),
                    );
                    Error::internal_error()
                })?;

            if let Some(tx_receipt) = tx_receipt.clone() {
                if tx_receipt.verified {
                    self.cache_of_transaction_receipts
                        .insert(tx_hash.as_ref().to_vec(), tx_receipt);
                }
            }

            tx_receipt
        };
        Ok(res)
    }

    async fn ticker_request(
        mut ticker_request_sender: mpsc::Sender<TickerRequest>,
        tx_type: TxFeeTypes,
        address: Address,
        token: TokenLike,
    ) -> Result<Fee> {
        let req = oneshot::channel();
        ticker_request_sender
            .send(TickerRequest::GetTxFee {
                tx_type: tx_type.clone(),
                address,
                token: token.clone(),
                response: req.0,
            })
            .await
            .expect("ticker receiver dropped");
        let resp = req.1.await.expect("ticker answer sender dropped");
        resp.map_err(|err| {
            log::warn!(
                "[{}:{}:{}] Internal Server Error: '{}'; input: {:?}, {:?}",
                file!(),
                line!(),
                column!(),
                err,
                tx_type,
                token,
            );
            Error::internal_error()
        })
    }

    async fn ticker_price_request(
        mut ticker_request_sender: mpsc::Sender<TickerRequest>,
        token: TokenLike,
        req_type: TokenPriceRequestType,
    ) -> Result<BigDecimal> {
        let req = oneshot::channel();
        ticker_request_sender
            .send(TickerRequest::GetTokenPrice {
                token: token.clone(),
                response: req.0,
                req_type,
            })
            .await
            .expect("ticker receiver dropped");
        let resp = req.1.await.expect("ticker answer sender dropped");
        resp.map_err(|err| {
            log::warn!(
                "[{}:{}:{}] Internal Server Error: '{}'; input: {:?}",
                file!(),
                line!(),
                column!(),
                err,
                token,
            );
            Error::internal_error()
        })
    }

    async fn get_account_state(&self, address: &Address) -> Result<AccountStateInfo> {
        let mut storage = self.access_storage().await?;
        let account_info = storage
            .chain()
            .account_schema()
            .account_state_by_address(address)
            .await
            .map_err(|_| Error::internal_error())?;

        let mut result = AccountStateInfo {
            account_id: None,
            committed: Default::default(),
            verified: Default::default(),
        };

        if let Some((account_id, commited_state)) = account_info.committed {
            result.account_id = Some(account_id);
            result.committed =
                ResponseAccountState::try_restore(commited_state, &self.token_cache).await?;
        };

        if let Some((_, verified_state)) = account_info.verified {
            result.verified =
                ResponseAccountState::try_restore(verified_state, &self.token_cache).await?;
        };

        Ok(result)
    }

    /// For forced exits, we must check that target account exists for more
    /// than 24 hours in order to give new account owners give an opportunity
    /// to set the signing key. While `ForcedExit` operation doesn't do anything
    /// bad to the account, it's more user-friendly to only allow this operation
    /// after we're somewhat sure that zkSync account is not owned by anybody.
    async fn check_forced_exit(&self, forced_exit: &zksync_types::ForcedExit) -> Result<()> {
        let target_account_address = forced_exit.target;
        let mut storage = self.access_storage().await?;
        let account_age = storage
            .chain()
            .operations_ext_schema()
            .account_created_on(&target_account_address)
            .await
            .map_err(|err| {
                vlog::warn!("Internal Server Error: '{}'; input: {:?}", err, forced_exit);
                Error::internal_error()
            })?;

        match account_age {
            Some(age) => {
                if (chrono::Utc::now() - age) >= self.forced_exit_minimum_account_age {
                    // Account does exist long enough, everything is OK.
                    Ok(())
                } else {
                    let err = format!(
                        "Target account exists less than required minimum amount ({} hours)",
                        self.forced_exit_minimum_account_age.num_hours()
                    );
                    Err(Error::invalid_params(err))
                }
            }
            None => Err(Error::invalid_params("Target account does not exist")),
        }
    }

    async fn eth_tx_for_withdrawal(&self, withdrawal_hash: TxHash) -> Result<Option<String>> {
        let res = if let Some(complete_withdrawals_tx_hash) = self
            .cache_of_complete_withdrawal_tx_hashes
            .get(&withdrawal_hash)
        {
            Some(complete_withdrawals_tx_hash)
        } else {
            let mut storage = self.access_storage().await?;
            let complete_withdrawals_tx_hash = storage
                .chain()
                .operations_schema()
                .eth_tx_for_withdrawal(&withdrawal_hash)
                .await
                .map_err(|err| {
                    vlog::warn!(
                        "Internal Server Error: '{}'; input: {:?}",
                        err,
                        withdrawal_hash,
                    );
                    Error::internal_error()
                })?
                .map(|tx_hash| format!("0x{}", hex::encode(&tx_hash)));

            if let Some(complete_withdrawals_tx_hash) = complete_withdrawals_tx_hash.clone() {
                self.cache_of_complete_withdrawal_tx_hashes
                    .insert(withdrawal_hash, complete_withdrawals_tx_hash);
            }

            complete_withdrawals_tx_hash
        };
        Ok(res)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn start_rpc_server(
    config_options: ConfigurationOptions,
    connection_pool: ConnectionPool,
    sign_verify_request_sender: mpsc::Sender<VerifyTxSignatureRequest>,
    ticker_request_sender: mpsc::Sender<TickerRequest>,
    panic_notify: mpsc::Sender<bool>,
) {
    let addr = config_options.json_rpc_http_server_address;

    let rpc_app = RpcApp::new(
        &config_options,
        connection_pool,
        sign_verify_request_sender,
        ticker_request_sender,
    );
    std::thread::spawn(move || {
        let _panic_sentinel = ThreadPanicNotify(panic_notify);
        let mut io = IoHandler::new();
        rpc_app.extend(&mut io);

        let server = ServerBuilder::new(io)
            .request_middleware(super::loggers::http_rpc::request_middleware)
            .threads(super::THREADS_PER_SERVER)
            .start_http(&addr)
            .unwrap();
        server.wait();
    });
}

async fn verify_tx_info_message_signature(
    tx: &ZkSyncTx,
    signature: Option<TxEthSignature>,
    msg_to_sign: Option<String>,
    mut req_channel: mpsc::Sender<VerifyTxSignatureRequest>,
) -> Result<VerifiedTx> {
    fn rpc_message(error: TxAddError) -> Error {
        Error {
            code: RpcErrorCodes::from(error).into(),
            message: error.to_string(),
            data: None,
        }
    }

    let eth_sign_data = match msg_to_sign {
        Some(message_to_sign) => {
            let signature =
                signature.ok_or_else(|| rpc_message(TxAddError::MissingEthSignature))?;

            Some(EthSignData {
                signature,
                message: message_to_sign,
            })
        }
        None => None,
    };

    let resp = oneshot::channel();

    let request = VerifyTxSignatureRequest {
        tx: tx.clone(),
        eth_sign_data,
        response: resp.0,
    };

    // Send the check request.
    req_channel.send(request).await.map_err(|err| {
        log::warn!(
            "[{}:{}:{}] Internal Server Error: '{}'; input: N/A",
            file!(),
            line!(),
            column!(),
            err
        );
        Error::internal_error()
    })?;

    // Wait for the check result.
    resp.1
        .await
        .map_err(|err| {
            log::warn!(
                "[{}:{}:{}] Internal Server Error: '{}'; input: N/A",
                file!(),
                line!(),
                column!(),
                err
            );
            Error::internal_error()
        })?
        .map_err(rpc_message)
}

#[cfg(test)]
mod test {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[test]
    fn tx_fee_type_serialization() {
        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct Query {
            tx_type: TxFeeTypes,
        }

        let cases = vec![
            (
                Query {
                    tx_type: TxFeeTypes::Withdraw,
                },
                r#"{"tx_type":"Withdraw"}"#,
            ),
            (
                Query {
                    tx_type: TxFeeTypes::Transfer,
                },
                r#"{"tx_type":"Transfer"}"#,
            ),
        ];
        for (query, json_str) in cases {
            let ser = serde_json::to_string(&query).expect("ser");
            assert_eq!(ser, json_str);
            let de = serde_json::from_str::<Query>(&ser).expect("de");
            assert_eq!(query, de);
        }
    }
}
