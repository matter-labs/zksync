#![allow(clippy::diverging_sub_expression)]
use std::cmp::max;
use std::collections::HashMap;
use std::sync::Arc;

use web3::types::{Address, BlockNumber};

use zksync_types::{
    AccountId, Deposit, FullExit, NewTokenEvent, Nonce, PriorityOp, RegisterNFTFactoryEvent,
    SerialId, TokenId, ZkSyncPriorityOp, H256,
};

use futures::channel::mpsc;
use futures::StreamExt;
use tokio::sync::RwLock;
use zksync_mempool::MempoolTransactionRequest;

use super::is_missing_priority_op_error;
use crate::eth_watch::{client::EthClient, EthWatch};

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
                .or_default()
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

    async fn set_last_block_number(&mut self, block_number: u64) {
        let mut inner = self.inner.write().await;
        inner.last_block_number = block_number;
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

fn create_watcher<T: EthClient>(
    client: T,
    mempool_tx_sender: mpsc::Sender<MempoolTransactionRequest>,
) -> EthWatch<T> {
    EthWatch::new(client, mempool_tx_sender, 1)
}

async fn fake_mempool(
    mut receiver: mpsc::Receiver<MempoolTransactionRequest>,
    data: Arc<RwLock<HashMap<SerialId, (PriorityOp, bool)>>>,
) {
    while let Some(a) = receiver.next().await {
        match a {
            MempoolTransactionRequest::NewTx(_, _) => {
                unreachable!()
            }
            MempoolTransactionRequest::NewPriorityOps(ops, conf, channel) => {
                for op in &ops {
                    let mut lock = data.write().await;
                    lock.insert(op.serial_id, (op.clone(), conf));
                }
                channel.send(Ok(())).unwrap_or_default()
            }
            MempoolTransactionRequest::NewTxsBatch(_, _, _) => unreachable!(),
        }
    }
}

#[tokio::test]
async fn test_operation_queues() {
    let mut client = FakeEthClient::new();

    let (sender, receiver) = mpsc::channel(10);
    let data = Arc::new(RwLock::new(HashMap::new()));
    tokio::spawn(fake_mempool(receiver, data.clone()));
    let from_addr = [1u8; 20].into();
    let to_addr = [2u8; 20].into();

    let priority_ops = vec![
        PriorityOp {
            serial_id: 0,
            data: ZkSyncPriorityOp::Deposit(Deposit {
                from: from_addr,
                token: TokenId(0),
                amount: Default::default(),
                to: Default::default(),
            }),
            deadline_block: 0,
            eth_hash: [2; 32].into(),
            eth_block: 3,
            eth_block_index: Some(1),
        },
        PriorityOp {
            serial_id: 1,
            data: ZkSyncPriorityOp::Deposit(Deposit {
                from: Default::default(),
                token: TokenId(0),
                amount: Default::default(),
                to: to_addr,
            }),
            deadline_block: 0,
            eth_hash: [3; 32].into(),
            eth_block: 4,
            eth_block_index: Some(1),
        },
        PriorityOp {
            serial_id: 2,
            data: ZkSyncPriorityOp::FullExit(FullExit {
                account_id: AccountId(1),
                eth_address: from_addr,
                token: TokenId(0),
                is_legacy: false,
            }),
            deadline_block: 0,
            eth_block: 4,
            eth_hash: [4; 32].into(),
            eth_block_index: Some(2),
        },
    ];

    client.add_operations(&priority_ops).await;

    let mut watcher = create_watcher(client, sender);
    watcher.poll_eth_node().await.unwrap();
    assert_eq!(watcher.eth_state.last_ethereum_block(), 4);

    let priority_queues = watcher.eth_state.priority_queue();
    let unconfirmed_queue = watcher.eth_state.unconfirmed_queue();
    assert_eq!(priority_queues.len(), 1);
    assert_eq!(
        priority_queues.values().next().unwrap().as_ref().serial_id,
        0
    );
    assert_eq!(unconfirmed_queue.len(), 2);
    assert_eq!(unconfirmed_queue[0].serial_id, 1);
    assert_eq!(unconfirmed_queue[1].serial_id, 2);

    priority_queues.get(&0).unwrap();
    let reader = data.read().await;
    let (op, confirmed) = reader.get(&priority_ops[0].serial_id).unwrap();
    assert_eq!(op.tx_hash(), priority_ops[0].tx_hash());
    assert!(confirmed);
    let (op, confirmed) = reader.get(&priority_ops[1].serial_id).unwrap();
    assert!(!confirmed);
    assert_eq!(op.tx_hash(), priority_ops[1].tx_hash());
    let (op, confirmed) = reader.get(&priority_ops[2].serial_id).unwrap();
    assert!(!confirmed);
    assert_eq!(op.tx_hash(), priority_ops[2].tx_hash());
}

/// This test simulates the situation when eth watch module did not poll Ethereum node for some time
/// (e.g. because of rate limit) and skipped more blocks than `number_of_confirmations_for_event`.
#[tokio::test]
async fn test_operation_queues_time_lag() {
    let mut client = FakeEthClient::new();

    // Below we initialize client with 3 operations: one for the 1st block, one for 100th, and one for 110th.
    // Client's block number will be 110, thus both first and second operations should get to the priority queue
    // in eth watcher.
    let (sender, receiver) = mpsc::channel(10);
    let data = Arc::new(RwLock::new(HashMap::new()));
    tokio::spawn(fake_mempool(receiver, data.clone()));
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
                eth_block_index: Some(1),
            },
            PriorityOp {
                serial_id: 1,
                data: ZkSyncPriorityOp::FullExit(FullExit {
                    account_id: AccountId(0),
                    eth_address: Default::default(),
                    token: TokenId(0),
                    is_legacy: false,
                }),
                deadline_block: 0,
                eth_hash: [3; 32].into(),
                eth_block: 100, // <-- Note 100th block, it will set the network block to 100.
                eth_block_index: Some(1),
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
                eth_block_index: Some(1),
            },
        ])
        .await;
    let mut watcher = create_watcher(client, sender);
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
    let (sender, receiver) = mpsc::channel(10);
    let data = Arc::new(RwLock::new(HashMap::new()));
    tokio::spawn(fake_mempool(receiver, data.clone()));
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
                eth_block_index: Some(1),
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
                eth_block_index: Some(1),
            },
        ])
        .await;

    let mut watcher = create_watcher(client.clone(), sender);
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
                eth_block_index: Some(1),
            },
            PriorityOp {
                serial_id: 4,
                data: ZkSyncPriorityOp::FullExit(FullExit {
                    account_id: AccountId(0),
                    eth_address: Default::default(),
                    token: TokenId(0),
                    is_legacy: false,
                }),
                deadline_block: 0,
                eth_hash: [3; 32].into(),
                eth_block: 5,
                eth_block_index: Some(2),
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

    let reader = data.read().await;
    let (op, confirmed) = reader.get(&3).unwrap();
    assert_eq!(op.eth_hash, H256::from_slice(&[2u8; 32]));
    assert!(!confirmed);
}

/// Checks that even for a big gap between skipped blocks, state is restored correctly.
#[tokio::test]
async fn test_restore_and_poll_time_lag() {
    let (sender, receiver) = mpsc::channel(10);
    let mut client = FakeEthClient::new();
    let data = Arc::new(RwLock::new(HashMap::new()));
    tokio::spawn(fake_mempool(receiver, data.clone()));
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
                eth_block_index: Some(1),
            },
            PriorityOp {
                serial_id: 1,
                data: ZkSyncPriorityOp::FullExit(FullExit {
                    account_id: AccountId(0),
                    eth_address: Default::default(),
                    token: TokenId(0),
                    is_legacy: false,
                }),
                deadline_block: 0,
                eth_hash: [3; 32].into(),
                eth_block: 100,
                eth_block_index: Some(1),
            },
        ])
        .await;

    let mut watcher = create_watcher(client.clone(), sender);
    watcher.restore_state_from_eth(101).await.unwrap();
    assert_eq!(watcher.eth_state.last_ethereum_block(), 101);
    let priority_queues = watcher.eth_state.priority_queue();
    assert_eq!(priority_queues.len(), 2);
    priority_queues.get(&0).unwrap();
    priority_queues.get(&1).unwrap();
}

#[tokio::test]
async fn test_serial_id_gaps() {
    let (sender, receiver) = mpsc::channel(10);
    let deposit = ZkSyncPriorityOp::Deposit(Deposit {
        from: Default::default(),
        token: TokenId(0),
        amount: Default::default(),
        to: [2u8; 20].into(),
    });

    let data = Arc::new(RwLock::new(HashMap::new()));
    tokio::spawn(fake_mempool(receiver, data.clone()));
    let mut client = FakeEthClient::new();
    client
        .add_operations(&[
            PriorityOp {
                serial_id: 0,
                data: deposit.clone(),
                deadline_block: 0,
                eth_hash: [2; 32].into(),
                eth_block: 1,
                eth_block_index: Some(1),
            },
            PriorityOp {
                serial_id: 1,
                data: deposit.clone(),
                deadline_block: 0,
                eth_hash: [3; 32].into(),
                eth_block: 1,
                eth_block_index: Some(2),
            },
        ])
        .await;

    let mut watcher = create_watcher(client.clone(), sender);
    // Restore the valid (empty) state.
    watcher.restore_state_from_eth(0).await.unwrap();
    assert_eq!(watcher.eth_state.last_ethereum_block(), 0);
    assert!(watcher.eth_state.priority_queue().is_empty());
    assert_eq!(watcher.eth_state.next_priority_op_id(), 0);

    // Advance the block number and poll the valid block range.
    client.set_last_block_number(2).await;
    watcher.poll_eth_node().await.unwrap();
    assert_eq!(watcher.eth_state.next_priority_op_id(), 2);
    assert_eq!(watcher.eth_state.last_ethereum_block_backup(), 0);
    assert_eq!(watcher.eth_state.last_ethereum_block(), 2);

    // Add a gap.
    client
        .add_operations(&[
            PriorityOp {
                serial_id: 2,
                data: deposit.clone(),
                deadline_block: 0,
                eth_hash: [2; 32].into(),
                eth_block: 2,
                eth_block_index: Some(1),
            },
            PriorityOp {
                serial_id: 4, // Then next id is expected to be 3.
                data: deposit.clone(),
                deadline_block: 0,
                eth_hash: [3; 32].into(),
                eth_block: 2,
                eth_block_index: Some(3),
            },
        ])
        .await;
    client.set_last_block_number(3).await;
    // Should detect a gap.
    let err = watcher.poll_eth_node().await.unwrap_err();
    assert!(is_missing_priority_op_error(&err));

    // The partially valid update is still discarded and we're waiting
    // for the serial_id = 2 even though it was present.
    assert_eq!(watcher.eth_state.next_priority_op_id(), 2);
    // The range got reset.
    assert_eq!(watcher.eth_state.last_ethereum_block_backup(), 0);
    assert_eq!(watcher.eth_state.last_ethereum_block(), 0);

    // Add a missing operations to the processed range.
    client
        .add_operations(&[PriorityOp {
            serial_id: 3,
            data: deposit.clone(),
            deadline_block: 0,
            eth_hash: [2; 32].into(),
            eth_block: 2,
            eth_block_index: Some(2),
        }])
        .await;
    watcher.poll_eth_node().await.unwrap();
    assert_eq!(watcher.eth_state.next_priority_op_id(), 5);
    assert_eq!(watcher.eth_state.priority_queue().len(), 5);
    assert_eq!(watcher.eth_state.last_ethereum_block_backup(), 0);
    assert_eq!(watcher.eth_state.last_ethereum_block(), 3);
}
