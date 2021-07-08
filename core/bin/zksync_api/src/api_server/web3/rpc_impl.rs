use std::time::Instant;
// External uses
use jsonrpc_core::Result;

use super::Web3RpcApp;

impl Web3RpcApp {
    pub async fn _impl_ping(self) -> Result<bool> {
        let start = Instant::now();
        metrics::histogram!("api.web3.ping", start.elapsed());
        Ok(true)
    }
}
