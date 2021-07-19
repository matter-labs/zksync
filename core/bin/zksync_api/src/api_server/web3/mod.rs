// Built-in uses
// External uses
use futures::channel::mpsc;
use jsonrpc_core::{Error, IoHandler, MetaIoHandler, Metadata, Middleware, Result};
use jsonrpc_http_server::ServerBuilder;
// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_storage::{
    chain::operations_ext::records::Web3TxReceipt, ConnectionPool, StorageProcessor,
};
use zksync_types::ZkSyncOp;
use zksync_utils::panic_notify::ThreadPanicNotify;
// Local uses
use self::{
    rpc_trait::Web3Rpc,
    types::{CommonLogData, LogsHelper, TransactionReceipt, H2048, H256, U256, U64},
};

mod converter;
mod rpc_impl;
mod rpc_trait;
mod types;

#[derive(Clone)]
pub struct Web3RpcApp {
    runtime_handle: tokio::runtime::Handle,
    connection_pool: ConnectionPool,
    logs_helper: LogsHelper,
    chain_id: u8,
}

impl Web3RpcApp {
    pub fn new(connection_pool: ConnectionPool, chain_id: u8) -> Self {
        let runtime_handle = tokio::runtime::Handle::try_current()
            .expect("Web3RpcApp must be created from the context of Tokio Runtime");

        Web3RpcApp {
            runtime_handle,
            connection_pool,
            logs_helper: LogsHelper::new(),
            chain_id,
        }
    }

    pub fn extend<T: Metadata, S: Middleware<T>>(self, io: &mut MetaIoHandler<T, S>) {
        io.extend_with(self.to_delegate())
    }
}

impl Web3RpcApp {
    async fn access_storage(&self) -> Result<StorageProcessor<'_>> {
        self.connection_pool
            .access_storage()
            .await
            .map_err(|_| Error::internal_error())
    }

    async fn block_transaction_count(
        storage: &mut StorageProcessor<'_>,
        block_number: zksync_types::BlockNumber,
    ) -> Result<U256> {
        let count = storage
            .chain()
            .block_schema()
            .get_block_transactions_count(block_number)
            .await
            .map_err(|_| Error::internal_error())?;
        Ok(U256::from(count))
    }

    async fn tx_receipt(
        &self,
        storage: &mut StorageProcessor<'_>,
        tx: Web3TxReceipt,
    ) -> Result<TransactionReceipt> {
        let root_hash = H256::from_slice(&tx.block_hash);
        let common_data: CommonLogData = tx.clone().into();
        let op: Option<ZkSyncOp> = serde_json::from_value(tx.operation).unwrap();
        let logs = if let Some(op) = op {
            let mut logs = self
                .logs_helper
                .erc_logs(op.clone(), common_data.clone(), storage)
                .await?;
            let zksync_log = self
                .logs_helper
                .zksync_log(op, common_data, storage)
                .await?;
            if let Some(zksync_log) = zksync_log {
                logs.push(zksync_log);
            }
            logs
        } else {
            Vec::new()
        };
        Ok(TransactionReceipt {
            transaction_hash: H256::from_slice(&tx.tx_hash),
            // U64::MAX for failed transactions
            transaction_index: tx.block_index.map(Into::into).unwrap_or(U64::MAX),
            block_hash: Some(root_hash),
            block_number: Some(tx.block_number.into()),
            cumulative_gas_used: 0.into(),
            gas_used: Some(0.into()),
            contract_address: None,
            logs,
            status: Some((tx.success as u8).into()),
            root: Some(root_hash),
            logs_bloom: H2048::zero(),
        })
    }
}

pub fn start_rpc_server(
    connection_pool: ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
    config: &ZkSyncConfig,
) {
    let addr = config.api.web3.bind_addr();

    let rpc_app = Web3RpcApp::new(connection_pool, config.eth_client.chain_id);
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
