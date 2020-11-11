pub(crate) mod utils;

use crate::data_restore_driver::DataRestoreDriver;
use crate::{END_ETH_BLOCKS_OFFSET, ETH_BLOCKS_STEP};
use db_test_macro::test as db_test;
use zksync_storage::{test_utils::create_data_restore_data, ConnectionPool, StorageProcessor};

use std::future::Future;
use web3::{RequestId, Transport};

#[derive(Debug, Clone)]
pub(crate) struct Web3Transport;

impl Transport for Web3Transport {
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

#[db_test]
async fn test_load_state_from_storage(mut storage: StorageProcessor<'_>) {
    create_data_restore_data(&mut storage).await;
    let mut driver = DataRestoreDriver::new(
        Web3Transport,
        [1u8; 20].into(),
        [1u8; 20].into(),
        ETH_BLOCKS_STEP,
        END_ETH_BLOCKS_OFFSET,
        vec![6, 30],
        true,
        None,
    );
    driver.load_state_from_storage(&mut storage).await;
}
