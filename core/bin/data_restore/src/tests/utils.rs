use std::future::Future;

use web3::{
    types::H256,
    types::{Bytes, Log},
    RequestId, Transport,
};

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

pub(crate) fn create_log(
    topic: H256,
    additional_topics: Vec<H256>,
    data: Bytes,
    block_number: u32,
) -> Log {
    let mut topics = vec![topic];
    topics.extend(additional_topics);
    Log {
        address: [1u8; 20].into(),
        topics,
        data,
        block_hash: None,
        block_number: Some(block_number.into()),
        transaction_hash: Some(u32_to_32bytes(1).into()),
        transaction_index: Some(0.into()),
        log_index: Some(0.into()),
        transaction_log_index: Some(0.into()),
        log_type: Some("mined".into()),
        removed: None,
    }
}
