use std::cmp::max;
use std::collections::HashMap;

use web3::types::{Address, BlockNumber};

use zksync_types::{
    AccountId, Deposit, FullExit, NewTokenEvent, Nonce, PriorityOp, RegisterNFTFactoryEvent,
    TokenId, ZkSyncPriorityOp,
};

use crate::eth_watch::{client::EthClient, EthWatch};
use std::sync::Arc;
use tokio::sync::RwLock;

struct FakeEthClientData {
    priority_ops: HashMap<u64, Vec<PriorityOp>>,
    last_block_number: u64,
}

impl FakeEthClientData {
    fn new() -> Self {
        Self {
            priority_ops: Default::default(),
            last_block_number: 0,
        }
    }

    fn add_operations(&mut self, ops: &[PriorityOp]) {
        for op in ops {
            self.last_block_number = max(op.eth_block, self.last_block_number);
            self.priority_ops
                .entry(op.eth_block)
                .or_insert_with(Vec::new)
                .push(op.clone());
        }
    }
}

#[derive(Clone)]
struct FakeEthClient {
    inner: Arc<RwLock<FakeEthClientData>>,
}

impl FakeEthClient {
    fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(FakeEthClientData::new())),
        }
    }

    async fn add_operations(&mut self, ops: &[PriorityOp]) {
        self.inner.write().await.add_operations(ops);
    }

    async fn block_to_number(&self, block: &BlockNumber) -> u64 {
        match block {
            BlockNumber::Latest => self.inner.read().await.last_block_number,
            BlockNumber::Earliest => 0,
            BlockNumber::Pending => unreachable!(),
            BlockNumber::Number(number) => number.as_u64(),
        }
    }
}

#[async_trait::async_trait]
impl EthClient for FakeEthClient {
    async fn get_priority_op_events(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<PriorityOp>, anyhow::Error> {
        let from = self.block_to_number(&from).await;
        let to = self.block_to_number(&to).await;
        let mut operations = vec![];
        for number in from..=to {
            if let Some(ops) = self.inner.read().await.priority_ops.get(&number) {
                operations.extend_from_slice(ops);
            }
        }
        Ok(operations)
    }

    async fn get_new_register_nft_factory_events(
        &self,
        _from: BlockNumber,
        _to: BlockNumber,
    ) -> anyhow::Result<Vec<RegisterNFTFactoryEvent>> {
        Ok(Vec::new())
    }

    async fn get_new_tokens_events(
        &self,
        _from: BlockNumber,
        _to: BlockNumber,
    ) -> anyhow::Result<Vec<NewTokenEvent>> {
        // Ignore NewTokens event.
        Ok(Vec::new())
    }

    async fn block_number(&self) -> Result<u64, anyhow::Error> {
        Ok(self.inner.read().await.last_block_number)
    }

    async fn get_auth_fact(
        &self,
        _address: Address,
        _nonce: Nonce,
    ) -> Result<Vec<u8>, anyhow::Error> {
        unreachable!()
    }

    async fn get_auth_fact_reset_time(
        &self,
        _address: Address,
        _nonce: Nonce,
    ) -> Result<u64, anyhow::Error> {
        unreachable!()
    }
}

fn create_watcher<T: EthClient>(client: T) -> EthWatch<T> {
    EthWatch::new(client, 1)
}

#[tokio::test]
async fn test_operation_queues() {
    let mut client = FakeEthClient::new();

    let from_addr = [1u8; 20].into();
    let to_addr = [2u8; 20].into();

    client
        .add_operations(&[
            PriorityOp {
                serial_id: 0,
                data: ZkSyncPriorityOp::Deposit(Deposit {
                    from: from_addr,
                    token: TokenId(0),
                    amount: Default::default(),
                    to: to_addr,
                }),
                deadline_block: 0,
                eth_hash: [2; 32].into(),
                eth_block: 4,
            },
            PriorityOp {
                serial_id: 1,
                data: ZkSyncPriorityOp::Deposit(Deposit {
                    from: Default::default(),
                    token: TokenId(0),
                    amount: Default::default(),
                    to: Default::default(),
                }),
                deadline_block: 0,
                eth_hash: [3; 32].into(),
                eth_block: 3,
            },
            PriorityOp {
                serial_id: 2,
                data: ZkSyncPriorityOp::FullExit(FullExit {
                    account_id: AccountId(1),
                    eth_address: from_addr,
                    token: TokenId(0),
                }),
                deadline_block: 0,
                eth_block: 4,
                eth_hash: [4; 32].into(),
            },
        ])
        .await;

    let mut watcher = create_watcher(client);
    watcher.poll_eth_node().await.unwrap();
    assert_eq!(watcher.eth_state.last_ethereum_block(), 4);

    let priority_queues = watcher.eth_state.priority_queue();
    let unconfirmed_queue = watcher.eth_state.unconfirmed_queue();
    assert_eq!(priority_queues.len(), 1);
    assert_eq!(
        priority_queues.values().next().unwrap().as_ref().serial_id,
        1
    );
    assert_eq!(unconfirmed_queue.len(), 2);
    assert_eq!(unconfirmed_queue[0].serial_id, 0);
    assert_eq!(unconfirmed_queue[1].serial_id, 2);

    priority_queues.get(&1).unwrap();
    watcher.find_ongoing_op_by_hash(&[2u8; 32]).unwrap();

    // Make sure that the old behavior of the pending deposits getter has not changed.
    let deposits = watcher.get_ongoing_deposits_for(to_addr);
    assert_eq!(deposits.len(), 1);
    // Check that the new pending operations getter shows only deposits with the same `from` address.
    let ops = watcher.get_ongoing_ops_for(from_addr);

    assert_eq!(ops[0].serial_id, 0);
    assert_eq!(ops[1].serial_id, 2);
    assert!(watcher.get_ongoing_ops_for(to_addr).is_empty());
}

/// This test simulates the situation when eth watch module did not poll Ethereum node for some time
/// (e.g. because of rate limit) and skipped more blocks than `number_of_confirmations_for_event`.
#[tokio::test]
async fn test_operation_queues_time_lag() {
    let mut client = FakeEthClient::new();

    // Below we initialize client with 3 operations: one for the 1st block, one for 100th, and one for 110th.
    // Client's block number will be 110, thus both first and second operations should get to the priority queue
    // in eth watcher.
    client
        .add_operations(&[
            PriorityOp {
                serial_id: 0,
                data: ZkSyncPriorityOp::Deposit(Deposit {
                    from: Default::default(),
                    token: TokenId(0),
                    amount: Default::default(),
                    to: [2u8; 20].into(),
                }),
                deadline_block: 0,
                eth_hash: [2; 32].into(),
                eth_block: 1, // <- First operation goes to the first block.
            },
            PriorityOp {
                serial_id: 1,
                data: ZkSyncPriorityOp::FullExit(FullExit {
                    account_id: AccountId(0),
                    eth_address: Default::default(),
                    token: TokenId(0),
                }),
                deadline_block: 0,
                eth_hash: [3; 32].into(),
                eth_block: 100, // <-- Note 100th block, it will set the network block to 100.
            },
            PriorityOp {
                serial_id: 2,
                data: ZkSyncPriorityOp::Deposit(Deposit {
                    from: Default::default(),
                    token: TokenId(0),
                    amount: Default::default(),
                    to: Default::default(),
                }),
                deadline_block: 0,
                eth_hash: [3; 32].into(),
                eth_block: 110, // <-- This operation will get to the unconfirmed queue.
            },
        ])
        .await;
    let mut watcher = create_watcher(client);
    watcher.poll_eth_node().await.unwrap();
    assert_eq!(watcher.eth_state.last_ethereum_block(), 110);

    let priority_queues = watcher.eth_state.priority_queue();
    let unconfirmed_queue = watcher.eth_state.unconfirmed_queue();
    assert_eq!(priority_queues.len(), 2, "Incorrect confirmed queue size");
    assert_eq!(
        unconfirmed_queue.len(),
        1,
        "Incorrect unconfirmed queue size"
    );
    assert_eq!(
        unconfirmed_queue[0].serial_id, 2,
        "Incorrect operation ID for the unconfirmed queue"
    );
    priority_queues
        .get(&0)
        .expect("Operation with serial ID 0 is not in the confirmed queue");
    priority_queues
        .get(&1)
        .expect("Operation with serial ID 1 is not in the confirmed queue");
}

#[tokio::test]
async fn test_restore_and_poll() {
    let mut client = FakeEthClient::new();
    client
        .add_operations(&[
            PriorityOp {
                serial_id: 0,
                data: ZkSyncPriorityOp::Deposit(Deposit {
                    from: Default::default(),
                    token: TokenId(0),
                    amount: Default::default(),
                    to: [2u8; 20].into(),
                }),
                deadline_block: 0,
                eth_hash: [2; 32].into(),
                eth_block: 4,
            },
            PriorityOp {
                serial_id: 1,
                data: ZkSyncPriorityOp::Deposit(Deposit {
                    from: Default::default(),
                    token: TokenId(0),
                    amount: Default::default(),
                    to: Default::default(),
                }),
                deadline_block: 0,
                eth_hash: [3; 32].into(),
                eth_block: 3,
            },
        ])
        .await;

    let mut watcher = create_watcher(client.clone());
    watcher.restore_state_from_eth(4).await.unwrap();
    client
        .add_operations(&[
            PriorityOp {
                serial_id: 3,
                data: ZkSyncPriorityOp::Deposit(Deposit {
                    from: Default::default(),
                    token: TokenId(0),
                    amount: Default::default(),
                    to: [2u8; 20].into(),
                }),
                deadline_block: 0,
                eth_hash: [2; 32].into(),
                eth_block: 5,
            },
            PriorityOp {
                serial_id: 4,
                data: ZkSyncPriorityOp::FullExit(FullExit {
                    account_id: AccountId(0),
                    eth_address: Default::default(),
                    token: TokenId(0),
                }),
                deadline_block: 0,
                eth_hash: [3; 32].into(),
                eth_block: 5,
            },
        ])
        .await;
    watcher.poll_eth_node().await.unwrap();
    assert_eq!(watcher.eth_state.last_ethereum_block(), 5);
    let priority_queues = watcher.eth_state.priority_queue();
    let unconfirmed_queue = watcher.eth_state.unconfirmed_queue();
    assert_eq!(priority_queues.len(), 2);
    assert_eq!(unconfirmed_queue.len(), 2);
    assert_eq!(unconfirmed_queue[0].serial_id, 3);
    priority_queues.get(&1).unwrap();
    watcher.find_ongoing_op_by_hash(&[2u8; 32]).unwrap();
    let deposits = watcher.get_ongoing_deposits_for([2u8; 20].into());
    assert_eq!(deposits.len(), 1);
}

/// Checks that even for a big gap between skipped blocks, state is restored correctly.
#[tokio::test]
async fn test_restore_and_poll_time_lag() {
    let mut client = FakeEthClient::new();
    client
        .add_operations(&[
            PriorityOp {
                serial_id: 0,
                data: ZkSyncPriorityOp::Deposit(Deposit {
                    from: Default::default(),
                    token: TokenId(0),
                    amount: Default::default(),
                    to: [2u8; 20].into(),
                }),
                deadline_block: 0,
                eth_hash: [2; 32].into(),
                eth_block: 1,
            },
            PriorityOp {
                serial_id: 1,
                data: ZkSyncPriorityOp::FullExit(FullExit {
                    account_id: AccountId(0),
                    eth_address: Default::default(),
                    token: TokenId(0),
                }),
                deadline_block: 0,
                eth_hash: [3; 32].into(),
                eth_block: 100,
            },
        ])
        .await;

    let mut watcher = create_watcher(client.clone());
    watcher.restore_state_from_eth(101).await.unwrap();
    assert_eq!(watcher.eth_state.last_ethereum_block(), 101);
    let priority_queues = watcher.eth_state.priority_queue();
    assert_eq!(priority_queues.len(), 2);
    priority_queues.get(&0).unwrap();
    priority_queues.get(&1).unwrap();
}
