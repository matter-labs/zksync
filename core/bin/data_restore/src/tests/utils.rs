use std::future::Future;
use web3::{RequestId, Transport};

// This transport is necessary for generating contract
#[derive(Debug, Clone)]
pub(crate) struct FakeTransport;

impl Transport for FakeTransport {
    type Out = Box<dyn Future<Output = Result<jsonrpc_core::Value, web3::Error>> + Send + Unpin>;

    fn prepare(
        &self,
        _method: &str,
        _params: Vec<jsonrpc_core::Value>,
    ) -> (RequestId, jsonrpc_core::Call) {
        unimplemented!()
    }

    fn send(&self, _id: RequestId, _request: jsonrpc_core::Call) -> Self::Out {
        unimplemented!()
    }
}

pub(crate) fn u32_to_32bytes(value: u32) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    let a = value.to_be_bytes();
    for i in 0..4 {
        bytes[28 + i] = a[i]
    }
    bytes
}
