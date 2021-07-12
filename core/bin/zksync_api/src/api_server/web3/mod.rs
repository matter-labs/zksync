// Built-in uses
use std::convert::TryInto;
// External uses
use futures::channel::mpsc;
use jsonrpc_core::{Error, IoHandler, MetaIoHandler, Metadata, Middleware, Result};
use jsonrpc_http_server::ServerBuilder;
// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_storage::{ConnectionPool, StorageProcessor};
use zksync_utils::panic_notify::ThreadPanicNotify;
// Local uses
use self::{rpc_trait::Web3Rpc, types::U256};

mod rpc_impl;
mod rpc_trait;
mod types;

#[derive(Clone)]
pub struct Web3RpcApp {
    runtime_handle: tokio::runtime::Handle,
    connection_pool: ConnectionPool,
    chain_id: u8,
}

impl Web3RpcApp {
    pub fn new(connection_pool: ConnectionPool, chain_id: u8) -> Self {
        let runtime_handle = tokio::runtime::Handle::try_current()
            .expect("Web3RpcApp must be created from the context of Tokio Runtime");

        Web3RpcApp {
            runtime_handle,
            connection_pool,
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

    async fn resolve_block_number(
        storage: &mut StorageProcessor<'_>,
        number: Option<self::types::BlockNumber>,
    ) -> Result<Option<zksync_types::BlockNumber>> {
        let last_saved_block = storage
            .chain()
            .block_schema()
            .get_last_saved_block()
            .await
            .map_err(|_| Error::internal_error())?;

        let number = match number {
            Some(number) => number,
            None => {
                return Ok(Some(last_saved_block));
            }
        };

        let number = match number {
            self::types::BlockNumber::Earliest => zksync_types::BlockNumber(0),
            self::types::BlockNumber::Committed => storage
                .chain()
                .block_schema()
                .get_last_committed_confirmed_block()
                .await
                .map_err(|_| Error::internal_error())?,
            self::types::BlockNumber::Finalized => storage
                .chain()
                .block_schema()
                .get_last_verified_confirmed_block()
                .await
                .map_err(|_| Error::internal_error())?,
            self::types::BlockNumber::Latest | self::types::BlockNumber::Pending => {
                last_saved_block
            }
            self::types::BlockNumber::Number(number) => {
                if number.as_u64() > last_saved_block.0 as u64 {
                    return Ok(None);
                }
                // Unwrap can be safely used because `number` is not greater than `last_saved_block`
                // which is `u32` variable.
                zksync_types::BlockNumber(number.as_u64().try_into().unwrap())
            }
        };
        Ok(Some(number))
    }

    async fn get_block_transaction_count(
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
}

#[allow(clippy::too_many_arguments)]
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
