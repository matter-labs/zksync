// Built-in uses
// External uses

use jsonrpc_core::{Error, IoHandler, MetaIoHandler, Metadata, Middleware, Result};
use jsonrpc_http_server::ServerBuilder;
// Workspace uses

use zksync_storage::{ConnectionPool, StorageProcessor};
use zksync_utils::panic_notify::{spawn_panic_handler, ThreadPanicNotify};
// Local uses
use self::{calls::CallsHelper, logs::LogsHelper, rpc_trait::Web3Rpc};

use tokio::task::JoinHandle;
use zksync_config::configs::api::{TokenConfig, Web3Config};
use zksync_types::ChainId;

mod calls;
mod converter;
mod logs;
mod rpc_impl;
mod rpc_trait;
#[cfg(test)]
mod tests;
mod types;

pub const ZKSYNC_PROXY_ADDRESS: &str = "1000000000000000000000000000000000000000";
pub const NFT_FACTORY_ADDRESS: &str = "2000000000000000000000000000000000000000";

#[derive(Clone)]
pub struct Web3RpcApp {
    connection_pool: ConnectionPool,
    logs_helper: LogsHelper,
    calls_helper: CallsHelper,
    max_block_range: u32,
    chain_id: ChainId,
}

impl Web3RpcApp {
    pub fn new(
        connection_pool: ConnectionPool,
        config: &Web3Config,
        token_config: &TokenConfig,
    ) -> Self {
        Web3RpcApp {
            connection_pool,
            logs_helper: LogsHelper::new(token_config.invalidate_token_cache_period()),
            calls_helper: CallsHelper::new(token_config.invalidate_token_cache_period()),
            max_block_range: config.max_block_range,
            chain_id: ChainId(config.chain_id),
        }
    }

    pub fn extend<T: Metadata, S: Middleware<T>>(self, io: &mut MetaIoHandler<T, S>) {
        io.extend_with(self.to_delegate())
    }

    async fn access_storage(&self) -> Result<StorageProcessor<'_>> {
        self.connection_pool
            .access_storage()
            .await
            .map_err(|_| Error::internal_error())
    }
}

pub fn start_rpc_server(
    connection_pool: ConnectionPool,
    web3_config: &Web3Config,
    token_config: &TokenConfig,
) -> JoinHandle<()> {
    let addr = web3_config.bind_addr();

    let rpc_app = Web3RpcApp::new(connection_pool, web3_config, token_config);
    let (handler, panic_sender) = spawn_panic_handler();

    std::thread::spawn(move || {
        let _panic_sentinel = ThreadPanicNotify(panic_sender);

        let mut io = IoHandler::new();
        rpc_app.extend(&mut io);

        let server = ServerBuilder::new(io)
            .threads(super::THREADS_PER_SERVER)
            .start_http(&addr)
            .unwrap();
        server.wait();
    });
    handler
}
