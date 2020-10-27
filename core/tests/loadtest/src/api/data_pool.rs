//! A pool of data required for api tests.

// Built-in uses
use std::{
    cmp::max,
    collections::{BTreeMap, HashMap, VecDeque},
    sync::Arc,
};
// External uses
use rand::{thread_rng, Rng};
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
// Workspace uses
use zksync_types::{tx::TxHash, Address, BlockNumber, PriorityOp, ZkSyncPriorityOp};
// Local uses

/// Maximum limit value in the requests.
const MAX_REQUEST_LIMIT: usize = 100;
/// The maximum number of items in queues to reduce memory consumption.
const MAX_QUEUE_LEN: usize = 100;

#[derive(Debug, Default, Clone)]
pub struct AddressData {
    /// Total count of transactions related to the address.
    pub txs_count: usize,
    /// Total count of priority operations related to the address.
    pub ops_count: usize,
}

/// Generates a `(offset, limit)` pair for the corresponding API request.
fn gen_offset_limit_pair(count: usize) -> (usize, usize) {
    let mut rng = thread_rng();
    // First argument of `gen_range` should be less than the second,
    // so if count is zero we should return zero to create a correct pair.
    let offset = rng.gen_range(0, max(1, count));
    // We can safely use any value in range `[0, MAX_REQUEST_LIMIT]` as the limit.
    let limit = rng.gen_range(1, MAX_REQUEST_LIMIT + 1);
    (offset, limit)
}

impl AddressData {
    /// Generates a `(offset, limit)` pair for transaction requests related to the address.
    pub fn gen_txs_offset_limit(&self) -> (usize, usize) {
        gen_offset_limit_pair(self.txs_count)
    }

    /// Generates a `(offset, limit)` pair for priority operation requests related to the address.
    pub fn gen_ops_offset_limit(&self) -> (usize, usize) {
        gen_offset_limit_pair(self.ops_count)
    }
}

// TODO In theory, we can use a simpler, fixed size deque instead of the standard one.

/// API data pool contents.
#[derive(Debug, Default)]
pub struct ApiDataPoolInner {
    addresses: Vec<Address>,
    data_by_address: HashMap<Address, AddressData>,
    txs: VecDeque<TxHash>,
    priority_ops: VecDeque<PriorityOp>,
    // Blocks with the counter of known transactions in them.
    blocks: BTreeMap<BlockNumber, usize>,
    max_block_number: BlockNumber,
}

impl ApiDataPoolInner {
    pub fn store_address(&mut self, address: Address) -> &mut AddressData {
        self.addresses.push(address);
        self.data_by_address.entry(address).or_default()
    }

    pub fn random_address(&self) -> (Address, &AddressData) {
        let idx = thread_rng().gen_range(0, self.addresses.len());
        let address = self.addresses[idx];
        (address, &self.data_by_address[&address])
    }

    pub fn store_tx_hash(&mut self, address: Address, tx_hash: TxHash) {
        self.txs.push_back(tx_hash);
        if self.txs.len() > MAX_QUEUE_LEN {
            self.txs.pop_front();
        }

        self.store_address(address).txs_count += 1;
    }

    pub fn random_tx_hash(&self) -> TxHash {
        let idx = thread_rng().gen_range(0, self.txs.len());
        self.txs[idx]
    }

    pub fn store_priority_op(&mut self, priority_op: PriorityOp) {
        if let ZkSyncPriorityOp::Deposit(deposit) = &priority_op.data {
            self.store_address(deposit.to).ops_count += 1;
        }

        self.priority_ops.push_back(priority_op);
        if self.priority_ops.len() > MAX_QUEUE_LEN {
            self.priority_ops.pop_front();
        }
    }

    pub fn random_priority_op(&self) -> PriorityOp {
        let idx = thread_rng().gen_range(0, self.priority_ops.len());
        self.priority_ops[idx].clone()
    }

    pub fn store_block(&mut self, number: BlockNumber) {
        self.max_block_number = max(self.max_block_number, number);
        // Update known transactions count in the block.
        *self.blocks.entry(number).or_default() += 1;

        if self.blocks.len() > MAX_QUEUE_LEN {
            // TODO: replace by the pop_first then the `map_first_last` becomes stable.
            let key = *self.blocks.keys().next().unwrap();
            self.blocks.remove(&key);
        }
    }

    /// Generates a random block number in range [0, max block number].
    pub fn random_block(&self) -> BlockNumber {
        self.random_tx_id().0
    }

    /// Generates a random transaction identifier (block number, position in block).
    pub fn random_tx_id(&self) -> (BlockNumber, usize) {
        let from = *self.blocks.keys().next().unwrap();
        let to = self.max_block_number;

        let mut rng = thread_rng();
        // Sometimes we have gaps in the block list, so it is not always
        // possible to randomly generate an existing block number.
        for _ in 0..MAX_REQUEST_LIMIT {
            let number = rng.gen_range(from, to + 1);
            if let Some(&block_txs) = self.blocks.get(&number) {
                let tx_id = rng.gen_range(0, block_txs);
                return (number, tx_id);
            }
        }

        unreachable!(
            "Unable to find the appropriate block number after {} attempts.",
            MAX_REQUEST_LIMIT
        );
    }
}

/// Provides needed data for the API load tests.
#[derive(Debug, Clone, Default)]
pub struct ApiDataPool {
    inner: Arc<RwLock<ApiDataPoolInner>>,
}

impl ApiDataPool {
    /// Max limit in the API requests with limit.
    pub const MAX_REQUEST_LIMIT: usize = MAX_REQUEST_LIMIT;

    /// Creates a new pool instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets readonly access to the pool content.
    pub async fn read(&self) -> RwLockReadGuard<'_, ApiDataPoolInner> {
        self.inner.read().await
    }

    /// Gets writeable access to the pool content.
    pub async fn write(&self) -> RwLockWriteGuard<'_, ApiDataPoolInner> {
        self.inner.write().await
    }
}
