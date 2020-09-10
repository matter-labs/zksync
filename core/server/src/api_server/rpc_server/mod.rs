use std::sync::{Arc, RwLock};
// External uses
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use jsonrpc_core::{Error, IoHandler, MetaIoHandler, Metadata, Middleware, Result};
use jsonrpc_http_server::ServerBuilder;
// Workspace uses
use models::{
    config_options::{ConfigurationOptions, ThreadPanicNotify},
    node::{
        tx::{TxEthSignature, TxHash},
        Address, FranklinTx, PriorityOp, Token, TokenId, TokenLike, TxFeeTypes,
    },
};
use storage::{
    chain::{
        block::records::BlockDetails, operations::records::StoredExecutedPriorityOperation,
        operations_ext::records::TxReceiptResponse,
    },
    ConnectionPool, StorageProcessor,
};
// Local uses
use crate::{
    api_server::ops_counter::ChangePubKeyOpsCounter,
    eth_watch::{EthBlockId, EthWatchRequest},
    fee_ticker::{Fee, TickerRequest},
    mempool::{MempoolRequest, TxAddError},
    signature_checker::{VerifiedTx, VerifyTxSignatureRequest},
    state_keeper::StateKeeperRequest,
    utils::{
        current_zksync_info::CurrentZksyncInfo, shared_lru_cache::SharedLruCache,
        token_db_cache::TokenDBCache,
    },
};
use bigdecimal::BigDecimal;
use models::node::tx::EthSignData;

mod error;
mod rpc_impl;
mod rpc_trait;
pub mod types;

use self::error::*;
pub use self::rpc_trait::Rpc;
use self::types::*;

pub(crate) async fn get_ongoing_priority_ops(
    eth_watcher_request_sender: &mpsc::Sender<EthWatchRequest>,
    address: Address,
) -> Result<Vec<(EthBlockId, PriorityOp)>> {
    let mut eth_watcher_request_sender = eth_watcher_request_sender.clone();

    let eth_watcher_response = oneshot::channel();

    // Get all the ongoing priority ops from the `EthWatcher`.
    eth_watcher_request_sender
        .send(EthWatchRequest::GetUnconfirmedDeposits {
            address,
            resp: eth_watcher_response.0,
        })
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
        })?;

    eth_watcher_response
        .1
        .await
        .map_err(|_| Error::internal_error())
}

#[derive(Clone)]
pub struct RpcApp {
    cache_of_executed_priority_operations: SharedLruCache<u32, StoredExecutedPriorityOperation>,
    cache_of_blocks_info: SharedLruCache<i64, BlockDetails>,
    cache_of_transaction_receipts: SharedLruCache<Vec<u8>, TxReceiptResponse>,

    pub mempool_request_sender: mpsc::Sender<MempoolRequest>,
    pub state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
    pub eth_watcher_request_sender: mpsc::Sender<EthWatchRequest>,
    pub sign_verify_request_sender: mpsc::Sender<VerifyTxSignatureRequest>,
    pub ticker_request_sender: mpsc::Sender<TickerRequest>,

    pub connection_pool: ConnectionPool,

    pub confirmations_for_eth_event: u64,
    pub token_cache: TokenDBCache,
    pub current_zksync_info: CurrentZksyncInfo,

    /// Counter for ChangePubKey operations to filter the spam.
    ops_counter: Arc<RwLock<ChangePubKeyOpsCounter>>,
}

impl RpcApp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config_options: &ConfigurationOptions,
        connection_pool: ConnectionPool,
        mempool_request_sender: mpsc::Sender<MempoolRequest>,
        state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
        sign_verify_request_sender: mpsc::Sender<VerifyTxSignatureRequest>,
        eth_watcher_request_sender: mpsc::Sender<EthWatchRequest>,
        ticker_request_sender: mpsc::Sender<TickerRequest>,
        current_zksync_info: CurrentZksyncInfo,
    ) -> Self {
        let token_cache = TokenDBCache::new(connection_pool.clone());

        let api_requests_caches_size = config_options.api_requests_caches_size;
        let confirmations_for_eth_event = config_options.confirmations_for_eth_event;

        RpcApp {
            cache_of_executed_priority_operations: SharedLruCache::new(api_requests_caches_size),
            cache_of_blocks_info: SharedLruCache::new(api_requests_caches_size),
            cache_of_transaction_receipts: SharedLruCache::new(api_requests_caches_size),

            connection_pool,

            mempool_request_sender,
            state_keeper_request_sender,
            sign_verify_request_sender,
            eth_watcher_request_sender,
            ticker_request_sender,

            confirmations_for_eth_event,
            token_cache,
            current_zksync_info,

            ops_counter: Arc::new(RwLock::new(ChangePubKeyOpsCounter::new())),
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
    async fn get_tx_info_message_to_sign(&self, tx: &FranklinTx) -> Result<Option<String>> {
        match tx {
            FranklinTx::Transfer(tx) => {
                let token = self.token_info_from_id(tx.token).await?;
                Ok(Some(
                    tx.get_ethereum_sign_message(&token.symbol, token.decimals),
                ))
            }
            FranklinTx::Withdraw(tx) => {
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
            .access_storage_fragile()
            .await
            .map_err(|_| Error::internal_error())
    }

    /// Async version of `get_ongoing_deposits` which does not use old futures as a return type.
    async fn get_ongoing_deposits_impl(&self, address: Address) -> Result<OngoingDepositsResp> {
        let confirmations_for_eth_event = self.confirmations_for_eth_event;

        let ongoing_ops =
            get_ongoing_priority_ops(&self.eth_watcher_request_sender, address).await?;

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
    ) -> Result<BigDecimal> {
        let req = oneshot::channel();
        ticker_request_sender
            .send(TickerRequest::GetTokenPrice {
                token: token.clone(),
                response: req.0,
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

    async fn get_verified_account_state(&self, address: &Address) -> Result<ResponseAccountState> {
        let mut storage = self.access_storage().await?;
        let account = storage
            .chain()
            .account_schema()
            .account_state_by_address(address)
            .await
            .map_err(|_| Error::internal_error())?;

        let verified_state = if let Some((_, account)) = account.verified {
            ResponseAccountState::try_restore(account, &self.token_cache).await?
        } else {
            Default::default()
        };

        Ok(verified_state)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn start_rpc_server(
    config_options: ConfigurationOptions,
    connection_pool: ConnectionPool,
    mempool_request_sender: mpsc::Sender<MempoolRequest>,
    state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
    sign_verify_request_sender: mpsc::Sender<VerifyTxSignatureRequest>,
    eth_watcher_request_sender: mpsc::Sender<EthWatchRequest>,
    ticker_request_sender: mpsc::Sender<TickerRequest>,
    panic_notify: mpsc::Sender<bool>,
    current_zksync_info: CurrentZksyncInfo,
) {
    let addr = config_options.json_rpc_http_server_address;
    std::thread::Builder::new()
        .name("json_rpc_http".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify);
            let mut io = IoHandler::new();

            let rpc_app = RpcApp::new(
                &config_options,
                connection_pool,
                mempool_request_sender,
                state_keeper_request_sender,
                sign_verify_request_sender,
                eth_watcher_request_sender,
                ticker_request_sender,
                current_zksync_info,
            );
            rpc_app.extend(&mut io);

            let server = ServerBuilder::new(io)
                .request_middleware(super::loggers::http_rpc::request_middleware)
                .threads(8)
                .start_http(&addr)
                .unwrap();

            server.wait();
        })
        .expect("JSON-RPC http thread");
}

async fn verify_tx_info_message_signature(
    tx: &FranklinTx,
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
