//! A pool of data required for api tests.

// Built-in uses
use std::{collections::VecDeque, sync::Arc};
// External uses
use rand::{thread_rng, Rng};
use tokio::sync::RwLock;
// Workspace uses
use zksync_types::{tx::TxHash, Address, BlockNumber, PriorityOp};
// Local uses

#[derive(Debug, Default)]
struct ApiDataPoolInner {
    addresses: Vec<Address>,
    txs: VecDeque<TxHash>,
    priority_ops: VecDeque<PriorityOp>,
    max_block_number: BlockNumber,
}

impl ApiDataPoolInner {
    // TODO use array deque.
    const MAX_QUEUE_LEN: usize = 100;

    fn store_address(&mut self, address: Address) {
        self.addresses.push(address)
    }

    fn random_address(&self) -> Address {
        let idx = thread_rng().gen_range(0, self.addresses.len());
        self.addresses[idx]
    }

    fn store_tx_hash(&mut self, tx_hash: TxHash) {
        self.txs.push_back(tx_hash);
        if self.txs.len() > Self::MAX_QUEUE_LEN {
            self.txs.pop_front();
        }
    }

    fn random_tx_hash(&self) -> TxHash {
        let idx = thread_rng().gen_range(0, self.txs.len());
        self.txs[idx]
    }

    fn store_priority_op(&mut self, priority_op: PriorityOp) {
        self.priority_ops.push_back(priority_op);
        if self.priority_ops.len() > Self::MAX_QUEUE_LEN {
            self.priority_ops.pop_front();
        }
    }

    fn random_priority_op(&self) -> PriorityOp {
        let idx = thread_rng().gen_range(0, self.priority_ops.len());
        self.priority_ops[idx].clone()
    }

    fn store_max_block_number(&mut self, number: BlockNumber) {
        self.max_block_number = std::cmp::max(self.max_block_number, number);
    }

    fn random_block_number(&self) -> BlockNumber {
        thread_rng().gen_range(0, self.max_block_number + 1)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ApiDataPool {
    inner: Arc<RwLock<ApiDataPoolInner>>,
}

impl ApiDataPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn store_address(&self, address: Address) {
        self.inner.write().await.store_address(address);
    }

    pub async fn random_address(&self) -> Address {
        self.inner.read().await.random_address()
    }

    pub async fn store_tx_hash(&self, tx_hash: TxHash) {
        self.inner.write().await.store_tx_hash(tx_hash);
    }

    pub async fn random_tx_hash(&self) -> TxHash {
        self.inner.read().await.random_tx_hash()
    }

    pub async fn store_priority_op(&self, priority_op: PriorityOp) {
        self.inner.write().await.store_priority_op(priority_op);
    }

    pub async fn random_priority_op(&self) -> PriorityOp {
        self.inner.read().await.random_priority_op()
    }

    pub async fn store_max_block_number(&self, number: BlockNumber) {
        self.inner.write().await.store_max_block_number(number);
    }

    pub async fn random_block_number(&self) -> BlockNumber {
        self.inner.read().await.random_block_number()
    }
}
