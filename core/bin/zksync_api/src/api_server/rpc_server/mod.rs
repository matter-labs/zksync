// Built-in uses
use std::time::Instant;

// External uses
use futures::channel::mpsc;
use jsonrpc_core::{Error, IoHandler, MetaIoHandler, Metadata, Middleware, Result};
use jsonrpc_http_server::ServerBuilder;
use tokio::task::JoinHandle;

// Workspace uses
use zksync_config::configs::api::{CommonApiConfig, JsonRpcConfig, TokenConfig};
use zksync_storage::{
    chain::{
        block::records::StorageBlockDetails, operations::records::StoredExecutedPriorityOperation,
        operations_ext::records::TxReceiptResponse,
    },
    ConnectionPool, StorageProcessor,
};
use zksync_types::{tx::TxHash, Address, BlockNumber, ChainId};
use zksync_utils::panic_notify::{spawn_panic_handler, ThreadPanicNotify};

// Local uses
use crate::{signature_checker::VerifySignatureRequest, utils::shared_lru_cache::AsyncLruCache};

pub mod error;
mod ip_insert_middleware;
mod rpc_impl;
mod rpc_trait;
pub mod types;

pub use self::rpc_trait::Rpc;
use self::types::*;
use super::tx_sender::TxSender;
use crate::fee_ticker::FeeTicker;
use ip_insert_middleware::IpInsertMiddleWare;
use zksync_mempool::MempoolTransactionRequest;

#[derive(Clone)]
pub struct RpcApp {
    cache_of_executed_priority_operations: AsyncLruCache<u32, StoredExecutedPriorityOperation>,
    cache_of_transaction_receipts: AsyncLruCache<Vec<u8>, TxReceiptResponse>,
    cache_of_complete_withdrawal_tx_hashes: AsyncLruCache<TxHash, String>,

    pub confirmations_for_eth_event: u64,

    tx_sender: TxSender,
}

impl RpcApp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        connection_pool: ConnectionPool,
        sign_verify_request_sender: mpsc::Sender<VerifySignatureRequest>,
        ticker: FeeTicker,
        config: &CommonApiConfig,
        token_config: &TokenConfig,
        confirmations_for_eth_event: u64,
        chain_id: ChainId,
        mempool_tx_sender: mpsc::Sender<MempoolTransactionRequest>,
    ) -> Self {
        let api_requests_caches_size = config.caches_size;

        let tx_sender = TxSender::new(
            connection_pool,
            sign_verify_request_sender,
            ticker,
            config,
            token_config,
            mempool_tx_sender,
            chain_id,
        );

        RpcApp {
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

        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "get_executed_priority_operation");
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
        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "get_block_info");
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

        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "get_tx_receipt");
        Ok(res)
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

        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "get_account_state");
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
                .map(|tx_hash| format!("0x{}", hex::encode(tx_hash)));

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
#[must_use]
pub fn start_rpc_server(
    connection_pool: ConnectionPool,
    sign_verify_request_sender: mpsc::Sender<VerifySignatureRequest>,
    ticker: FeeTicker,
    config: &JsonRpcConfig,
    common_api_config: &CommonApiConfig,
    token_config: &TokenConfig,
    mempool_tx_sender: mpsc::Sender<MempoolTransactionRequest>,
    chain_id: ChainId,
    confirmations_for_eth_event: u64,
) -> JoinHandle<()> {
    let addr = config.http_bind_addr();
    let rpc_app = RpcApp::new(
        connection_pool,
        sign_verify_request_sender,
        ticker,
        common_api_config,
        token_config,
        confirmations_for_eth_event,
        chain_id,
        mempool_tx_sender,
    );

    let (handler, panic_sender) = spawn_panic_handler();
    std::thread::spawn(move || {
        let _panic_sentinel = ThreadPanicNotify(panic_sender);
        let mut io = IoHandler::new();
        rpc_app.extend(&mut io);

        let server = ServerBuilder::new(io)
            .threads(super::THREADS_PER_SERVER)
            .request_middleware(IpInsertMiddleWare {})
            .start_http(&addr)
            .unwrap();
        server.wait();
    });
    handler
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};
    use zksync_types::TxFeeTypes;

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
