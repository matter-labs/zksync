use std::future::Future;

use web3::{
    types::{Bytes, Log},
    types::{H160, H256},
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
        unreachable!()
    }

    fn send(&self, _id: RequestId, _request: jsonrpc_core::Call) -> Self::Out {
        unreachable!()
    }
}

pub(crate) fn u32_to_32bytes(value: u32) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    let bytes_value = value.to_be_bytes();
    // Change only the last 4 bytes, which are represent u32
    bytes[28..32].clone_from_slice(&bytes_value);
    bytes
}

pub(crate) fn create_log(
    address: H160,
    topic: H256,
    additional_topics: Vec<H256>,
    data: Bytes,
    block_number: u32,
    transaction_hash: H256,
) -> Log {
    let mut topics = vec![topic];
    topics.extend(additional_topics);
    Log {
        address,
        topics,
        data,
        block_hash: None,
        block_number: Some(block_number.into()),
        transaction_hash: Some(transaction_hash),
        transaction_index: Some(0.into()),
        log_index: Some(0.into()),
        transaction_log_index: Some(0.into()),
        log_type: Some("mined".into()),
        removed: None,
    }
}
