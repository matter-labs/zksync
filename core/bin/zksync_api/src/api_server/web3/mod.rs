// Built-in uses
// External uses
use futures::channel::mpsc;
use jsonrpc_core::{IoHandler, MetaIoHandler, Metadata, Middleware};
use jsonrpc_http_server::ServerBuilder;

// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_storage::ConnectionPool;
use zksync_utils::panic_notify::ThreadPanicNotify;
// Local uses
use self::rpc_trait::Web3Rpc;

mod rpc_impl;
mod rpc_trait;

#[derive(Clone)]
pub struct Web3RpcApp {
    connection_pool: ConnectionPool,
    runtime_handle: tokio::runtime::Handle,
}

impl Web3RpcApp {
    pub fn new(connection_pool: ConnectionPool) -> Self {
        let runtime_handle = tokio::runtime::Handle::try_current()
            .expect("Web3RpcApp must be created from the context of Tokio Runtime");

        Web3RpcApp {
            connection_pool,
            runtime_handle,
        }
    }

    pub fn extend<T: Metadata, S: Middleware<T>>(self, io: &mut MetaIoHandler<T, S>) {
        io.extend_with(self.to_delegate())
    }
}

#[allow(clippy::too_many_arguments)]
pub fn start_rpc_server(
    connection_pool: ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
    config: &ZkSyncConfig,
) {
    let addr = config.api.web3.bind_addr();

    let rpc_app = Web3RpcApp::new(connection_pool);
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
