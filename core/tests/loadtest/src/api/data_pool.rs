//! A pool of data required for api tests.

// Built-in uses
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
// External uses
use rand::{thread_rng, Rng};
use tokio::sync::RwLock;
// Workspace uses
use zksync_types::{tx::TxHash, Address, BlockNumber, PriorityOp, ZkSyncPriorityOp};
// Local uses

#[derive(Debug, Default, Copy, Clone)]
pub struct AddressData {
    pub txs_count: usize,
    pub ops_count: usize,
}

impl AddressData {
    const MAX_LIMIT: usize = 100;

    pub fn gen_txs_offset_limit(&self) -> (usize, usize) {
        let mut rng = thread_rng();

        let offset = rng.gen_range(0, std::cmp::max(1, self.txs_count));
        let limit = rng.gen_range(0, std::cmp::max(Self::MAX_LIMIT, offset));
        (offset, limit)
    }

    pub fn gen_ops_offset_limit(&self) -> (usize, usize) {
        let mut rng = thread_rng();

        let offset = rng.gen_range(0, std::cmp::max(1, self.ops_count));
        let limit = rng.gen_range(0, std::cmp::max(Self::MAX_LIMIT, offset));
        (offset, limit)
    }
}

#[derive(Debug, Default)]
struct ApiDataPoolInner {
    addresses: Vec<Address>,
    data_by_address: HashMap<Address, AddressData>,
    txs: VecDeque<TxHash>,
    priority_ops: VecDeque<PriorityOp>,
    max_block_number: BlockNumber,
}

impl ApiDataPoolInner {
    // TODO use array deque.
    const MAX_QUEUE_LEN: usize = 100;

    fn store_address(&mut self, address: Address) -> &mut AddressData {
        self.addresses.push(address);
        self.data_by_address.entry(address).or_default()
    }

    fn random_address(&self) -> (Address, AddressData) {
        let idx = thread_rng().gen_range(0, self.addresses.len());
        let address = self.addresses[idx];
        (address, self.data_by_address[&address])
    }

    fn store_tx_hash(&mut self, address: Address, tx_hash: TxHash) {
        self.txs.push_back(tx_hash);
        if self.txs.len() > Self::MAX_QUEUE_LEN {
            self.txs.pop_front();
        }

        self.store_address(address).txs_count += 1;
    }

    fn random_tx_hash(&self) -> TxHash {
        let idx = thread_rng().gen_range(0, self.txs.len());
        self.txs[idx]
    }

    fn store_priority_op(&mut self, priority_op: PriorityOp) {
        if let ZkSyncPriorityOp::Deposit(deposit) = &priority_op.data {
            self.store_address(deposit.to).ops_count += 1;
        }

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

    pub async fn random_address(&self) -> (Address, AddressData) {
        self.inner.read().await.random_address()
    }

    pub async fn store_tx_hash(&self, address: Address, tx_hash: TxHash) {
        self.inner.write().await.store_tx_hash(address, tx_hash);
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
