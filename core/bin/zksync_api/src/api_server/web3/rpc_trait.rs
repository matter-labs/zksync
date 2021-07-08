// External uses
use futures::{FutureExt, TryFutureExt};
use jsonrpc_core::Error;
use jsonrpc_derive::rpc;

// Local uses
use super::Web3RpcApp;

pub type FutureResp<T> = Box<dyn futures01::Future<Item = T, Error = Error> + Send>;

#[rpc]
pub trait Web3Rpc {
    #[rpc(name = "ping", returns = "bool")]
    fn ping(&self) -> FutureResp<bool>;
}

impl Web3Rpc for Web3RpcApp {
    fn ping(&self) -> FutureResp<bool> {
        let handle = self.runtime_handle.clone();
        let self_ = self.clone();
        let resp = async move { handle.spawn(self_._impl_ping()).await.unwrap() };
        Box::new(resp.boxed().compat())
    }
}
