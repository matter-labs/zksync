use std::cmp::max;
use std::collections::HashMap;

use web3::types::{Address, BlockNumber};

use zksync_types::{ethereum::CompleteWithdrawalsTx, Deposit, Log, PriorityOp, ZkSyncPriorityOp};

use crate::eth_watch::{client::EthClient, storage::Storage, EthWatch};

struct FakeStorage {
    withdrawal_txs: Vec<CompleteWithdrawalsTx>,
}
impl FakeStorage {
    fn new() -> Self {
        Self {
            withdrawal_txs: vec![],
        }
    }
}

#[async_trait::async_trait]
impl Storage for FakeStorage {
    async fn store_complete_withdrawals(
        &mut self,
        complete_withdrawals_txs: Vec<CompleteWithdrawalsTx>,
    ) -> anyhow::Result<()> {
        self.withdrawal_txs.extend(complete_withdrawals_txs);
        Ok(())
    }
}

struct FakeEthClient {
    priority_ops: HashMap<u64, Vec<PriorityOp>>,
    withdrawals: HashMap<u64, Vec<CompleteWithdrawalsTx>>,
    last_block_number: u64,
    // pending_withdrawals: HashMap<u64, Log>,
}
impl FakeEthClient {
    fn new() -> Self {
        Self {
            priority_ops: Default::default(),
            withdrawals: Default::default(),
            last_block_number: 0,
            // pending_withdrawals: Default::default(),
        }
    }
    fn add_operations(&mut self, ops: &[PriorityOp]) {
        for op in ops {
            self.last_block_number = max(op.eth_block, self.last_block_number);
            self.priority_ops
                .entry(op.eth_block)
                .or_insert(vec![])
                .push(op.clone());
        }
    }
    fn block_to_number(&self, block: &BlockNumber) -> u64 {
        match block {
            BlockNumber::Latest => self.last_block_number,
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
        let from = self.block_to_number(&from);
        let to = self.block_to_number(&to);
        let mut operations = vec![];
        for number in from..=to {
            if let Some(ops) = self.priority_ops.get(&number) {
                operations.extend_from_slice(ops);
            }
        }
        Ok(operations)
    }

    async fn get_complete_withdrawals_event(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<CompleteWithdrawalsTx>, anyhow::Error> {
        let from = self.block_to_number(&from);
        let to = self.block_to_number(&to);
        let mut withdrawals = vec![];
        for number in from..=to {
            if let Some(ops) = self.withdrawals.get(&number) {
                withdrawals.extend_from_slice(ops);
            }
        }
        Ok(withdrawals)
    }

    async fn block_number(&self) -> Result<u64, anyhow::Error> {
        Ok(self.last_block_number)
    }

    async fn get_auth_fact(
        &self,
        _address: Address,
        _nonce: u32,
    ) -> Result<Vec<u8>, anyhow::Error> {
        unimplemented!()
    }

    async fn get_first_pending_withdrawal_index(&self) -> Result<u32, anyhow::Error> {
        unimplemented!()
    }

    async fn get_number_of_pending_withdrawals(&self) -> Result<u32, anyhow::Error> {
        unimplemented!()
    }
}

fn create_watcher(client: FakeEthClient) -> EthWatch<FakeEthClient, FakeStorage> {
    let storage = FakeStorage::new();
    EthWatch::new(client, storage, 1)
}

#[tokio::test]
async fn test_operation_queues() {
    let mut client = FakeEthClient::new();
    client.add_operations(&vec![
        PriorityOp {
            serial_id: 0,
            data: ZkSyncPriorityOp::Deposit(Deposit {
                from: Default::default(),
                token: 0,
                amount: Default::default(),
                to: [2u8; 20].into(),
            }),
            deadline_block: 0,
            eth_hash: vec![1, 2, 3, 4, 5],
            eth_block: 4,
        },
        PriorityOp {
            serial_id: 1,
            data: ZkSyncPriorityOp::Deposit(Deposit {
                from: Default::default(),
                token: 0,
                amount: Default::default(),
                to: Default::default(),
            }),
            deadline_block: 0,
            eth_hash: vec![6, 7, 8, 9],
            eth_block: 3,
        },
    ]);
    let mut watcher = create_watcher(client);
    watcher.poll_eth_node().await.unwrap();
    assert_eq!(watcher.eth_state.last_ethereum_block(), 4);
    let priority_queues = watcher.eth_state.priority_queue();
    let unconfirmed_queue = watcher.eth_state.unconfirmed_queue();
    assert_eq!(priority_queues.len(), 1);
    assert_eq!(unconfirmed_queue.len(), 1);
    assert_eq!(unconfirmed_queue[0].serial_id, 0);
    priority_queues.get(&1).unwrap();
    watcher.find_ongoing_op_by_hash(&[1, 2, 3, 4, 5]).unwrap();
    let deposits = watcher.get_ongoing_deposits_for([2u8; 20].into());
    assert_eq!(deposits.len(), 1);
}
