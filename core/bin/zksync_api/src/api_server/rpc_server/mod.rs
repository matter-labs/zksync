// Built-in uses
use std::time::Instant;

// External uses
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use jsonrpc_core::{Error, IoHandler, MetaIoHandler, Metadata, Middleware, Result};
use jsonrpc_http_server::ServerBuilder;

// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_storage::{
    chain::{
        block::records::StorageBlockDetails, operations::records::StoredExecutedPriorityOperation,
        operations_ext::records::TxReceiptResponse,
    },
    ConnectionPool, StorageProcessor,
};
use zksync_types::{tx::TxHash, Address, BlockNumber, TokenLike, TxFeeTypes};

// Local uses
use crate::{
    fee_ticker::{PriceError, ResponseBatchFee, ResponseFee, TickerRequest, TokenPriceRequestType},
    signature_checker::VerifySignatureRequest,
    utils::shared_lru_cache::AsyncLruCache,
};
use bigdecimal::BigDecimal;
use zksync_utils::panic_notify::ThreadPanicNotify;

pub mod error;
mod rpc_impl;
mod rpc_trait;
pub mod types;

pub use self::rpc_trait::Rpc;
use self::types::*;
use super::tx_sender::TxSender;

#[derive(Clone)]
pub struct RpcApp {
    runtime_handle: tokio::runtime::Handle,

    cache_of_executed_priority_operations: AsyncLruCache<u32, StoredExecutedPriorityOperation>,
    cache_of_transaction_receipts: AsyncLruCache<Vec<u8>, TxReceiptResponse>,
    cache_of_complete_withdrawal_tx_hashes: AsyncLruCache<TxHash, String>,

    pub confirmations_for_eth_event: u64,

    tx_sender: TxSender,
}

impl RpcApp {
    pub fn new(
        connection_pool: ConnectionPool,
        sign_verify_request_sender: mpsc::Sender<VerifySignatureRequest>,
        ticker_request_sender: mpsc::Sender<TickerRequest>,
        config: &ZkSyncConfig,
    ) -> Self {
        let runtime_handle = tokio::runtime::Handle::try_current()
            .expect("RpcApp must be created from the context of Tokio Runtime");

        let api_requests_caches_size = config.api.common.caches_size;
        let confirmations_for_eth_event = config.eth_watch.confirmations_for_eth_event;

        let tx_sender = TxSender::new(
            connection_pool,
            sign_verify_request_sender,
            ticker_request_sender,
            config,
        );

        RpcApp {
            runtime_handle,

            cache_of_executed_priority_operations: AsyncLruCache::new(api_requests_caches_size),
            cache_of_transaction_receipts: AsyncLruCache::new(api_requests_caches_size),
            cache_of_complete_withdrawal_tx_hashes: AsyncLruCache::new(api_requests_caches_size),

            confirmations_for_eth_event,

            tx_sender,
        }
    }

    pub fn extend<T: Metadata, S: Middleware<T>>(self, io: &mut MetaIoHandler<T, S>) {
        io.extend_with(self.to_delegate())
    }
}

impl RpcApp {
    async fn access_storage(&self) -> Result<StorageProcessor<'_>> {
        self.tx_sender
            .pool
            .access_storage()
            .await
            .map_err(|_| Error::internal_error())
    }

    /// Async version of `get_ongoing_deposits` which does not use old futures as a return type.
    async fn get_ongoing_deposits_impl(&self, address: Address) -> Result<OngoingDepositsResp> {
        let start = Instant::now();
        let confirmations_for_eth_event = self.confirmations_for_eth_event;

        let ongoing_ops = self
            .tx_sender
            .core_api_client
            .get_unconfirmed_deposits(address)
            .await
            .map_err(|_| Error::internal_error())?;

        let mut max_block_number = 0;

        // Transform operations into `OngoingDeposit` and find the maximum block number in a
        // single pass.
        let deposits: Vec<_> = ongoing_ops
            .into_iter()
            .map(|op| {
                if op.eth_block > max_block_number {
                    max_block_number = op.eth_block;
                }

                OngoingDeposit::new(op)
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

        metrics::histogram!("api.rpc.get_ongoing_deposits", start.elapsed());
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
        let start = Instant::now();
        let res = if let Some(executed_op) = self
            .cache_of_executed_priority_operations
            .get(&serial_id)
            .await
        {
            Some(executed_op)
        } else {
            let mut storage = self.access_storage().await?;
            let executed_op = storage
                .chain()
                .operations_schema()
                .get_executed_priority_operation(serial_id)
                .await
                .map_err(|err| {
                    vlog::warn!("Internal Server Error: '{}'; input: {}", err, serial_id);
                    Error::internal_error()
                })?;

            if let Some(executed_op) = executed_op.clone() {
                self.cache_of_executed_priority_operations
                    .insert(serial_id, executed_op)
                    .await;
            }

            executed_op
        };

        metrics::histogram!("api.rpc.get_executed_priority_operation", start.elapsed());
        Ok(res)
    }

    async fn get_block_info(&self, block_number: i64) -> Result<Option<StorageBlockDetails>> {
        let start = Instant::now();
        let res = self
            .tx_sender
            .blocks
            .get(&self.tx_sender.pool, BlockNumber(block_number as u32))
            .await
            .map_err(|_| Error::internal_error())?;
        metrics::histogram!("api.rpc.get_block_info", start.elapsed());
        Ok(res)
    }

    async fn get_tx_receipt(&self, tx_hash: TxHash) -> Result<Option<TxReceiptResponse>> {
        let start = Instant::now();
        let res = if let Some(tx_receipt) = self
            .cache_of_transaction_receipts
            .get(&tx_hash.as_ref().to_vec())
            .await
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
                    vlog::warn!(
                        "Internal Server Error: '{}'; input: {}",
                        err,
                        tx_hash.to_string()
                    );
                    Error::internal_error()
                })?;

            if let Some(tx_receipt) = tx_receipt.clone() {
                if tx_receipt.verified {
                    self.cache_of_transaction_receipts
                        .insert(tx_hash.as_ref().to_vec(), tx_receipt)
                        .await;
                }
            }

            tx_receipt
        };

        metrics::histogram!("api.rpc.get_tx_receipt", start.elapsed());
        Ok(res)
    }

    async fn token_allowed_for_fees(
        mut ticker_request_sender: mpsc::Sender<TickerRequest>,
        token: TokenLike,
    ) -> Result<bool> {
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
            .map_err(|err| {
                vlog::warn!("Internal Server Error: '{}'; input: {:?}", err, token);
                Error::internal_error()
            })
    }

    async fn ticker_batch_fee_request(
        mut ticker_request_sender: mpsc::Sender<TickerRequest>,
        transactions: Vec<(TxFeeTypes, Address)>,
        token: TokenLike,
    ) -> Result<ResponseBatchFee> {
        let req = oneshot::channel();
        ticker_request_sender
            .send(TickerRequest::GetBatchTxFee {
                transactions,
                token: token.clone(),
                response: req.0,
            })
            .await
            .expect("ticker receiver dropped");
        let resp = req.1.await.expect("ticker answer sender dropped");
        resp.map_err(|err| {
            vlog::warn!("Internal Server Error: '{}'; input: {:?}", err, token,);
            Error::internal_error()
        })
    }

    async fn ticker_request(
        mut ticker_request_sender: mpsc::Sender<TickerRequest>,
        tx_type: TxFeeTypes,
        address: Address,
        token: TokenLike,
    ) -> Result<ResponseFee> {
        let req = oneshot::channel();
        ticker_request_sender
            .send(TickerRequest::GetTxFee {
                tx_type,
                address,
                token: token.clone(),
                response: req.0,
            })
            .await
            .expect("ticker receiver dropped");
        let resp = req.1.await.expect("ticker answer sender dropped");
        resp.map_err(|err| {
            vlog::warn!(
                "Internal Server Error: '{}'; input: {:?}, {:?}",
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
        resp.map_err(|err| match err {
            PriceError::TokenNotFound(msg) => Error::invalid_params(msg),
            _ => {
                vlog::warn!("Internal Server Error: '{}'; input: {:?}", err, token);
                Error::internal_error()
            }
        })
    }

    async fn get_account_state(&self, address: Address) -> Result<AccountStateInfo> {
        let start = Instant::now();
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

        if let Some((account_id, committed_state)) = account_info.committed {
            result.account_id = Some(account_id);
            result.committed = ResponseAccountState::try_restore(
                &mut storage,
                &self.tx_sender.tokens,
                committed_state,
            )
            .await?;
        };

        if let Some((_, verified_state)) = account_info.verified {
            result.verified = ResponseAccountState::try_restore(
                &mut storage,
                &self.tx_sender.tokens,
                verified_state,
            )
            .await?;
        };

        metrics::histogram!("api.rpc.get_account_state", start.elapsed());
        Ok(result)
    }

    async fn eth_tx_for_withdrawal(&self, withdrawal_hash: TxHash) -> Result<Option<String>> {
        let res = if let Some(complete_withdrawals_tx_hash) = self
            .cache_of_complete_withdrawal_tx_hashes
            .get(&withdrawal_hash)
            .await
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
                    .insert(withdrawal_hash, complete_withdrawals_tx_hash)
                    .await;
            }

            complete_withdrawals_tx_hash
        };
        Ok(res)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn start_rpc_server(
    connection_pool: ConnectionPool,
    sign_verify_request_sender: mpsc::Sender<VerifySignatureRequest>,
    ticker_request_sender: mpsc::Sender<TickerRequest>,
    panic_notify: mpsc::Sender<bool>,
    config: &ZkSyncConfig,
) {
    let addr = config.api.json_rpc.http_bind_addr();

    let rpc_app = RpcApp::new(
        connection_pool,
        sign_verify_request_sender,
        ticker_request_sender,
        &config,
    );
    std::thread::spawn(move || {
        let _panic_sentinel = ThreadPanicNotify(panic_notify);
        let mut io = IoHandler::new();
        rpc_app.extend(&mut io);

        let server = ServerBuilder::new(io)
            .threads(super::THREADS_PER_SERVER)
            .start_http(&addr)
            .unwrap();
        server.wait();
    });
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
