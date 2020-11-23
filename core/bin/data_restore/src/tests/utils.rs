use std::future::Future;

use crate::data_restore_driver::StorageUpdateState;
use crate::events::BlockEvent;
use crate::events_state::{EventsState, NewTokenEvent};
use crate::rollup_ops::RollupOpsBlock;
use crate::storage_interactor::StorageInteractor;
use web3::{
    types::H256,
    types::{Bytes, Log},
    RequestId, Transport,
};
use zksync_types::block::Block;
use zksync_types::{AccountMap, AccountUpdate, AccountUpdates, TokenGenesisListItem};

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
    topic: H256,
    additional_topics: Vec<H256>,
    data: Bytes,
    block_number: u32,
    transaction_hash: H256,
) -> Log {
    let mut topics = vec![topic];
    topics.extend(additional_topics);
    Log {
        address: [1u8; 20].into(),
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

struct InMemoryStorageInteractor {}

impl StorageInteractor for InMemoryStorageInteractor {
    async fn save_rollup_ops(&mut self, blocks: &[RollupOpsBlock]) {
        unimplemented!()
    }

    async fn update_tree_state(&mut self, block: Block, accounts_updated: AccountUpdates) {
        unimplemented!()
    }

    async fn store_token(&mut self, token: TokenGenesisListItem, token_id: u16) {
        unimplemented!()
    }

    async fn save_events_state(
        &mut self,
        block_events: &[BlockEvent],
        tokens: &[NewTokenEvent],
        last_watched_eth_block_number: u64,
    ) {
        unimplemented!()
    }

    async fn save_genesis_tree_state(&mut self, genesis_acc_update: AccountUpdate) {
        unimplemented!()
    }

    async fn get_block_events_state_from_storage(&mut self) -> EventsState {
        unimplemented!()
    }

    async fn get_tree_state(&mut self) -> (u32, AccountMap, u64, u32) {
        unimplemented!()
    }

    async fn get_ops_blocks_from_storage(&mut self) -> Vec<RollupOpsBlock> {
        unimplemented!()
    }

    async fn update_eth_state(&mut self) {
        unimplemented!()
    }

    async fn get_storage_state(&mut self) -> StorageUpdateState {
        unimplemented!()
    }
}
